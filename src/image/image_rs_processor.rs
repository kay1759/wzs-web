//! # Image Processor Implementation (`image` crate)
//!
//! Provides an [`ImageProcessor`] implementation backed by the [`image`] crate.
//!
//! This processor supports:
//! - `image/jpeg`
//! - `image/jpg`
//! - `image/png`
//! - `image/gif`
//!
//! and implements the resize behaviors defined by [`ResizeMode`]:
//! - [`ResizeMode::Fit`]
//! - [`ResizeMode::Contain`]
//! - [`ResizeMode::Cover`]
//!
//! # Safety
//!
//! This implementation includes basic protections against oversized or
//! malicious images:
//!
//! - maximum compressed input size (`max_input_bytes`)
//! - maximum sniffed width
//! - maximum sniffed height
//! - maximum sniffed total pixel count
//!
//! These checks are performed before full decode whenever possible.
//!
//! # EXIF Orientation
//!
//! For JPEG input, this processor reads EXIF orientation and normalizes the
//! decoded image before resizing. This avoids common smartphone rotation issues.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::str::FromStr;
//! use wzs_web::image::image_rs_processor::ImageRsProcessor;
//! use wzs_web::image::processor::{BgColor, ImageProcessor, ResizeMode, ResizeOpts};
//!
//! let processor = ImageRsProcessor::default();
//! let img_data = std::fs::read("input.jpg").unwrap();
//!
//! let opts = ResizeOpts::new(
//!     800,
//!     600,
//!     true,
//!     ResizeMode::Contain,
//!     BgColor::from_str("#ffffffff").unwrap(),
//! );
//!
//! if processor.is_supported("image/jpeg") {
//!     let resized = processor
//!         .resize_same_format(&img_data, "image/jpeg", opts)
//!         .expect("resize ok");
//!     std::fs::write("resized.jpg", resized).unwrap();
//! }
//! ```

use std::io::Cursor;

use anyhow::{bail, Context, Result};
use exif::{In, Reader as ExifReader, Tag};
use image::{
    imageops::{self, FilterType},
    ColorType, DynamicImage, GenericImageView, ImageFormat, ImageReader, Rgba,
};

use super::processor::{BgColor, ImageProcessor, ResizeMode, ResizeOpts};

/// Decode/input safety limits used to mitigate oversized images and
/// decompression-bomb-style attacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeLimits {
    /// Maximum allowed compressed input bytes.
    pub max_input_bytes: usize,
    /// Maximum allowed source width.
    pub max_width: u32,
    /// Maximum allowed source height.
    pub max_height: u32,
    /// Maximum allowed source pixel count (`width * height`).
    pub max_pixels: u64,
}

impl DecodeLimits {
    /// Creates a new set of decode limits.
    pub const fn new(
        max_input_bytes: usize,
        max_width: u32,
        max_height: u32,
        max_pixels: u64,
    ) -> Self {
        Self {
            max_input_bytes,
            max_width,
            max_height,
            max_pixels,
        }
    }

    fn validate_input_size(&self, img_bytes: &[u8]) -> Result<()> {
        if img_bytes.len() > self.max_input_bytes {
            bail!(
                "input image too large: {} bytes exceeds limit {} bytes",
                img_bytes.len(),
                self.max_input_bytes
            );
        }
        Ok(())
    }

    fn validate_dimensions(&self, width: u32, height: u32) -> Result<()> {
        if width > self.max_width {
            bail!(
                "image width too large: {width} exceeds limit {}",
                self.max_width
            );
        }
        if height > self.max_height {
            bail!(
                "image height too large: {height} exceeds limit {}",
                self.max_height
            );
        }

        let pixels = (width as u64) * (height as u64);
        if pixels > self.max_pixels {
            bail!(
                "image pixel count too large: {pixels} exceeds limit {}",
                self.max_pixels
            );
        }

        Ok(())
    }
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            // 20 MiB compressed input
            max_input_bytes: 20 * 1024 * 1024,
            // Large enough for typical uploads, small enough to reject absurd images
            max_width: 12_000,
            max_height: 12_000,
            max_pixels: 40_000_000,
        }
    }
}

/// Concrete [`ImageProcessor`] implementation using the `image` crate.
#[derive(Clone, Debug)]
pub struct ImageRsProcessor {
    limits: DecodeLimits,
}

impl Default for ImageRsProcessor {
    fn default() -> Self {
        Self {
            limits: DecodeLimits::default(),
        }
    }
}

impl ImageRsProcessor {
    /// Creates a processor with explicit decode/input limits.
    pub const fn new(limits: DecodeLimits) -> Self {
        Self { limits }
    }

    /// Returns the configured decode limits.
    pub const fn limits(&self) -> DecodeLimits {
        self.limits
    }

    /// Returns `true` if the given MIME type is supported by this processor.
    pub fn is_supported(&self, content_type: &str) -> bool {
        matches!(
            content_type.to_ascii_lowercase().as_str(),
            "image/gif" | "image/jpeg" | "image/jpg" | "image/png"
        )
    }

    /// Resizes the image and re-encodes it in the same format as requested by `content_type`.
    pub fn resize_same_format(
        &self,
        img_bytes: &[u8],
        content_type: &str,
        opts: ResizeOpts,
    ) -> Result<Vec<u8>> {
        let output_format = output_format_from_content_type(content_type)?;
        self.limits.validate_input_size(img_bytes)?;

        let (src_w, src_h) = sniff_dimensions(img_bytes).context("read image dimensions")?;
        self.limits
            .validate_dimensions(src_w, src_h)
            .context("validate image dimensions")?;

        let img = decode_image(img_bytes).context("decode image bytes")?;
        let img = maybe_normalize_orientation(img_bytes, content_type, img);

        let processed = process_image(img, opts);
        encode_same_format(processed, output_format).context("encode resized image")
    }
}

impl ImageProcessor for ImageRsProcessor {
    fn is_supported(&self, content_type: &str) -> bool {
        Self::is_supported(self, content_type)
    }

    fn resize_same_format(
        &self,
        img_bytes: &[u8],
        content_type: &str,
        opts: ResizeOpts,
    ) -> Result<Vec<u8>> {
        Self::resize_same_format(self, img_bytes, content_type, opts)
    }
}

fn output_format_from_content_type(content_type: &str) -> Result<ImageFormat> {
    match content_type.to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => Ok(ImageFormat::Jpeg),
        "image/png" => Ok(ImageFormat::Png),
        "image/gif" => Ok(ImageFormat::Gif),
        _ => bail!("unsupported content-type: {content_type}"),
    }
}

fn sniff_dimensions(img_bytes: &[u8]) -> Result<(u32, u32)> {
    ImageReader::new(Cursor::new(img_bytes))
        .with_guessed_format()
        .context("guess image format from bytes")?
        .into_dimensions()
        .context("extract image dimensions")
}

fn decode_image(img_bytes: &[u8]) -> Result<DynamicImage> {
    ImageReader::new(Cursor::new(img_bytes))
        .with_guessed_format()
        .context("guess image format from bytes")?
        .decode()
        .context("decode image data")
}

fn encode_same_format(img: DynamicImage, format: ImageFormat) -> Result<Vec<u8>> {
    let (w, h) = img.dimensions();
    let mut out = Vec::new();
    let mut cursor = Cursor::new(&mut out);

    match format {
        ImageFormat::Jpeg => {
            let rgb = img.to_rgb8();
            image::write_buffer_with_format(
                &mut cursor,
                &rgb,
                w,
                h,
                ColorType::Rgb8,
                ImageFormat::Jpeg,
            )?;
        }
        ImageFormat::Png => {
            let rgba = img.to_rgba8();
            image::write_buffer_with_format(
                &mut cursor,
                &rgba,
                w,
                h,
                ColorType::Rgba8,
                ImageFormat::Png,
            )?;
        }
        ImageFormat::Gif => {
            let rgba = img.to_rgba8();
            DynamicImage::ImageRgba8(rgba).write_to(&mut cursor, ImageFormat::Gif)?;
        }
        _ => bail!("unsupported output format: {format:?}"),
    }

    Ok(out)
}

fn process_image(img: DynamicImage, opts: ResizeOpts) -> DynamicImage {
    let (src_w, src_h) = img.dimensions();
    let already_within_bounds = src_w <= opts.max_w && src_h <= opts.max_h;

    if already_within_bounds && !opts.upscale {
        return img;
    }

    match opts.resize_mode {
        ResizeMode::Fit => resize_fit(img, opts.max_w, opts.max_h, opts.upscale),
        ResizeMode::Contain => resize_contain(
            img,
            opts.max_w,
            opts.max_h,
            opts.upscale,
            bg_color_to_rgba(opts.bg_color),
        ),
        ResizeMode::Cover => resize_cover(img, opts.max_w, opts.max_h, opts.upscale),
    }
}

fn bg_color_to_rgba(color: BgColor) -> Rgba<u8> {
    Rgba([color.r, color.g, color.b, color.a])
}

/// Keeps aspect ratio and fits entirely within the target box.
fn resize_fit(img: DynamicImage, max_w: u32, max_h: u32, upscale: bool) -> DynamicImage {
    let (w, h) = img.dimensions();

    if !upscale && w <= max_w && h <= max_h {
        return img;
    }

    img.resize(max_w, max_h, FilterType::Triangle)
}

/// Keeps aspect ratio, fits entirely within the target box, and pads the
/// remaining area to produce an exact `max_w x max_h` output.
fn resize_contain(
    img: DynamicImage,
    max_w: u32,
    max_h: u32,
    upscale: bool,
    bg: Rgba<u8>,
) -> DynamicImage {
    let fitted = resize_fit(img, max_w, max_h, upscale);
    let (fw, fh) = fitted.dimensions();

    if fw == max_w && fh == max_h {
        return fitted;
    }

    let mut canvas = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(max_w, max_h, bg));
    let x = ((max_w - fw) / 2) as i64;
    let y = ((max_h - fh) / 2) as i64;
    imageops::overlay(&mut canvas, &fitted, x, y);
    canvas
}

/// Keeps aspect ratio, fills the full target box, and crops overflow from the center.
fn resize_cover(img: DynamicImage, max_w: u32, max_h: u32, upscale: bool) -> DynamicImage {
    let (w, h) = img.dimensions();

    if !upscale && w <= max_w && h <= max_h {
        return img;
    }

    let scale_w = max_w as f32 / w as f32;
    let scale_h = max_h as f32 / h as f32;
    let scale = scale_w.max(scale_h);

    let new_w = ((w as f32) * scale).round() as u32;
    let new_h = ((h as f32) * scale).round() as u32;

    let resized = img.resize_exact(new_w, new_h, FilterType::Triangle);

    let crop_x = (new_w.saturating_sub(max_w)) / 2;
    let crop_y = (new_h.saturating_sub(max_h)) / 2;

    resized.crop_imm(crop_x, crop_y, max_w, max_h)
}

fn maybe_normalize_orientation(
    img_bytes: &[u8],
    content_type: &str,
    img: DynamicImage,
) -> DynamicImage {
    match content_type.to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => match read_exif_orientation(img_bytes) {
            Some(orientation) => apply_orientation(img, orientation),
            None => img,
        },
        _ => img,
    }
}

fn read_exif_orientation(img_bytes: &[u8]) -> Option<u16> {
    let mut cursor = Cursor::new(img_bytes);
    let exif = ExifReader::new().read_from_container(&mut cursor).ok()?;
    let field = exif.get_field(Tag::Orientation, In::PRIMARY)?;
    field.value.get_uint(0).map(|v| v as u16)
}

fn apply_orientation(img: DynamicImage, orientation: u16) -> DynamicImage {
    match orientation {
        1 => img,
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.fliph().rotate90(),
        6 => img.rotate90(),
        7 => img.fliph().rotate270(),
        8 => img.rotate270(),
        _ => img,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer, Rgba};

    fn make_pattern_rgba(width: u32, height: u32) -> image::RgbaImage {
        ImageBuffer::from_fn(width, height, |x, y| {
            if (x + y) % 2 == 0 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 255, 0, 255])
            }
        })
    }

    fn make_center_block_rgba(
        width: u32,
        height: u32,
        bg: [u8; 4],
        fg: [u8; 4],
        block_w: u32,
        block_h: u32,
    ) -> image::RgbaImage {
        let mut img = ImageBuffer::from_pixel(width, height, Rgba(bg));
        let start_x = (width.saturating_sub(block_w)) / 2;
        let start_y = (height.saturating_sub(block_h)) / 2;

        for y in start_y..(start_y + block_h).min(height) {
            for x in start_x..(start_x + block_w).min(width) {
                img.put_pixel(x, y, Rgba(fg));
            }
        }

        img
    }

    fn make_orientation_probe_rgba() -> image::RgbaImage {
        let mut img = ImageBuffer::from_pixel(3, 2, Rgba([0, 0, 0, 255]));
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255])); // top-left red
        img.put_pixel(2, 0, Rgba([0, 255, 0, 255])); // top-right green
        img.put_pixel(0, 1, Rgba([0, 0, 255, 255])); // bottom-left blue
        img.put_pixel(2, 1, Rgba([255, 255, 0, 255])); // bottom-right yellow
        img
    }

    fn encode_png(img: &image::RgbaImage) -> Vec<u8> {
        let mut cur = Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut cur,
            img.as_raw(),
            img.width(),
            img.height(),
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .expect("encode png");
        cur.into_inner()
    }

    fn encode_gif(img: &image::RgbaImage) -> Vec<u8> {
        let dyn_img = DynamicImage::ImageRgba8(img.clone());
        let mut cur = Cursor::new(Vec::new());
        dyn_img
            .write_to(&mut cur, image::ImageFormat::Gif)
            .expect("encode gif");
        cur.into_inner()
    }

    fn decode_dims(bytes: &[u8]) -> (u32, u32) {
        image::load_from_memory(bytes)
            .expect("decode image")
            .dimensions()
    }

    fn decode_rgba(bytes: &[u8]) -> image::RgbaImage {
        image::load_from_memory(bytes)
            .expect("decode image")
            .to_rgba8()
    }

    fn assert_jpeg_signature(bytes: &[u8]) {
        assert!(bytes.len() >= 3, "jpeg output too short");
        assert_eq!(bytes[0], 0xFF);
        assert_eq!(bytes[1], 0xD8);
        assert_eq!(bytes[2], 0xFF);
    }

    fn assert_png_signature(bytes: &[u8]) {
        assert!(bytes.len() >= 8, "png output too short");
        assert_eq!(&bytes[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    fn assert_gif_signature(bytes: &[u8]) {
        assert!(bytes.len() >= 6, "gif output too short");
        assert!(
            &bytes[0..6] == b"GIF87a" || &bytes[0..6] == b"GIF89a",
            "unexpected gif header: {:?}",
            &bytes[0..6]
        );
    }

    #[test]
    fn decode_limits_default_is_sane() {
        let limits = DecodeLimits::default();
        assert!(limits.max_input_bytes > 0);
        assert!(limits.max_width > 0);
        assert!(limits.max_height > 0);
        assert!(limits.max_pixels > 0);
    }

    #[test]
    fn decode_limits_reject_large_input_bytes() {
        let limits = DecodeLimits::new(3, 100, 100, 10_000);
        let err = limits
            .validate_input_size(&[0, 1, 2, 3])
            .expect_err("must reject oversize input");
        assert!(err.to_string().contains("input image too large"));
    }

    #[test]
    fn decode_limits_reject_large_dimensions() {
        let limits = DecodeLimits::new(1024, 100, 100, 10_000);

        let err = limits
            .validate_dimensions(101, 50)
            .expect_err("must reject large width");
        assert!(err.to_string().contains("image width too large"));

        let err = limits
            .validate_dimensions(50, 101)
            .expect_err("must reject large height");
        assert!(err.to_string().contains("image height too large"));

        let err = limits
            .validate_dimensions(101, 101)
            .expect_err("must reject too many pixels");
        assert!(
            err.to_string().contains("image width too large")
                || err.to_string().contains("image pixel count too large")
        );
    }

    #[test]
    fn supports_expected_mimes() {
        let p = ImageRsProcessor::default();
        assert!(p.is_supported("image/png"));
        assert!(p.is_supported("image/jpeg"));
        assert!(p.is_supported("image/jpg"));
        assert!(p.is_supported("image/gif"));

        assert!(p.is_supported("IMAGE/PNG"));
        assert!(p.is_supported("Image/Jpeg"));

        assert!(!p.is_supported("image/webp"));
        assert!(!p.is_supported("text/plain"));
        assert!(!p.is_supported("application/octet-stream"));
    }

    #[test]
    fn output_format_mapping_accepts_supported_types() {
        assert_eq!(
            output_format_from_content_type("image/jpeg").unwrap(),
            ImageFormat::Jpeg
        );
        assert_eq!(
            output_format_from_content_type("image/jpg").unwrap(),
            ImageFormat::Jpeg
        );
        assert_eq!(
            output_format_from_content_type("image/png").unwrap(),
            ImageFormat::Png
        );
        assert_eq!(
            output_format_from_content_type("image/gif").unwrap(),
            ImageFormat::Gif
        );
    }

    #[test]
    fn output_format_mapping_rejects_unsupported_types() {
        let err = output_format_from_content_type("image/webp").expect_err("must reject webp");
        assert!(err.to_string().contains("unsupported content-type"));

        let err =
            output_format_from_content_type("text/plain").expect_err("must reject text/plain");
        assert!(err.to_string().contains("unsupported content-type"));
    }

    #[test]
    fn sniff_dimensions_reads_dimensions_without_full_decode() {
        let src = encode_png(&make_pattern_rgba(123, 45));
        let dims = sniff_dimensions(&src).expect("sniff dimensions");
        assert_eq!(dims, (123, 45));
    }

    #[test]
    fn fit_downscales_within_bounds_and_preserves_aspect_ratio() {
        let p = ImageRsProcessor::default();
        let src = encode_png(&make_pattern_rgba(2000, 1000));

        let out = p
            .resize_same_format(
                &src,
                "image/jpeg",
                ResizeOpts::new(1280, 1280, false, ResizeMode::Fit, BgColor::white()),
            )
            .expect("resize ok");

        assert_jpeg_signature(&out);

        let (rw, rh) = decode_dims(&out);
        assert!(rw <= 1280 && rh <= 1280, "actual dims: {rw}x{rh}");

        let ratio = rw as f64 / rh as f64;
        assert!(
            (ratio - 2.0).abs() < 0.05,
            "expected aspect ratio ~2.0, got {ratio}"
        );
    }

    #[test]
    fn fit_does_not_upscale_when_disabled() {
        let p = ImageRsProcessor::default();
        let src = encode_png(&make_pattern_rgba(100, 50));

        let out = p
            .resize_same_format(
                &src,
                "image/png",
                ResizeOpts::new(500, 500, false, ResizeMode::Fit, BgColor::white()),
            )
            .expect("resize ok");

        assert_png_signature(&out);
        assert_eq!(decode_dims(&out), (100, 50));
    }

    #[test]
    fn fit_upscales_when_enabled() {
        let p = ImageRsProcessor::default();
        let src = encode_png(&make_pattern_rgba(100, 50));

        let out = p
            .resize_same_format(
                &src,
                "image/png",
                ResizeOpts::new(500, 500, true, ResizeMode::Fit, BgColor::white()),
            )
            .expect("resize ok");

        let (rw, rh) = decode_dims(&out);
        assert_eq!((rw, rh), (500, 250));
    }

    #[test]
    fn contain_downscales_and_outputs_exact_canvas_size() {
        let p = ImageRsProcessor::default();
        let src = encode_png(&make_pattern_rgba(2000, 1000));

        let out = p
            .resize_same_format(
                &src,
                "image/png",
                ResizeOpts::new(
                    500,
                    500,
                    false,
                    ResizeMode::Contain,
                    BgColor::new(255, 255, 255, 255),
                ),
            )
            .expect("resize ok");

        assert_png_signature(&out);
        assert_eq!(decode_dims(&out), (500, 500));
    }

    #[test]
    fn contain_upscales_and_outputs_exact_canvas_size() {
        let p = ImageRsProcessor::default();
        let src = encode_png(&make_pattern_rgba(100, 50));

        let out = p
            .resize_same_format(
                &src,
                "image/png",
                ResizeOpts::new(
                    500,
                    500,
                    true,
                    ResizeMode::Contain,
                    BgColor::new(255, 255, 255, 255),
                ),
            )
            .expect("resize ok");

        assert_eq!(decode_dims(&out), (500, 500));
    }

    #[test]
    fn contain_uses_requested_background_color() {
        let p = ImageRsProcessor::default();
        let src = encode_png(&make_pattern_rgba(200, 100));
        let bg = BgColor::new(10, 20, 30, 255);

        let out = p
            .resize_same_format(
                &src,
                "image/png",
                ResizeOpts::new(400, 400, true, ResizeMode::Contain, bg),
            )
            .expect("resize ok");

        let decoded = decode_rgba(&out);
        assert_eq!(decoded.dimensions(), (400, 400));

        let top_left = decoded.get_pixel(0, 0);
        assert_eq!(*top_left, Rgba([10, 20, 30, 255]));
    }

    #[test]
    fn contain_preserves_transparent_background_for_png() {
        let p = ImageRsProcessor::default();
        let src = encode_png(&make_pattern_rgba(200, 100));

        let out = p
            .resize_same_format(
                &src,
                "image/png",
                ResizeOpts::new(400, 400, true, ResizeMode::Contain, BgColor::transparent()),
            )
            .expect("resize ok");

        let decoded = decode_rgba(&out);
        assert_eq!(decoded.dimensions(), (400, 400));

        let top_left = decoded.get_pixel(0, 0);
        assert_eq!(*top_left, Rgba([0, 0, 0, 0]));
    }

    #[test]
    fn contain_returns_original_when_small_and_upscale_is_false() {
        let p = ImageRsProcessor::default();
        let src = encode_png(&make_pattern_rgba(200, 100));

        let out = p
            .resize_same_format(
                &src,
                "image/png",
                ResizeOpts::new(
                    400,
                    400,
                    false,
                    ResizeMode::Contain,
                    BgColor::new(10, 20, 30, 255),
                ),
            )
            .expect("resize ok");

        assert_eq!(decode_dims(&out), (200, 100));
    }

    #[test]
    fn cover_downscales_and_outputs_exact_canvas_size() {
        let p = ImageRsProcessor::default();
        let src = encode_png(&make_pattern_rgba(2000, 1000));

        let out = p
            .resize_same_format(
                &src,
                "image/png",
                ResizeOpts::new(500, 500, false, ResizeMode::Cover, BgColor::transparent()),
            )
            .expect("resize ok");

        assert_eq!(decode_dims(&out), (500, 500));
    }

    #[test]
    fn cover_upscales_when_enabled() {
        let p = ImageRsProcessor::default();
        let src = encode_png(&make_pattern_rgba(100, 50));

        let out = p
            .resize_same_format(
                &src,
                "image/png",
                ResizeOpts::new(500, 500, true, ResizeMode::Cover, BgColor::transparent()),
            )
            .expect("resize ok");

        assert_eq!(decode_dims(&out), (500, 500));
    }

    #[test]
    fn cover_returns_original_when_small_and_upscale_is_false() {
        let p = ImageRsProcessor::default();
        let src = encode_png(&make_pattern_rgba(200, 100));

        let out = p
            .resize_same_format(
                &src,
                "image/png",
                ResizeOpts::new(400, 400, false, ResizeMode::Cover, BgColor::transparent()),
            )
            .expect("resize ok");

        assert_eq!(decode_dims(&out), (200, 100));
    }

    #[test]
    fn cover_crops_from_center() {
        let p = ImageRsProcessor::default();

        let src_img = make_center_block_rgba(400, 200, [0, 0, 0, 255], [255, 0, 0, 255], 80, 80);
        let src = encode_png(&src_img);

        let out = p
            .resize_same_format(
                &src,
                "image/png",
                ResizeOpts::new(100, 100, false, ResizeMode::Cover, BgColor::transparent()),
            )
            .expect("resize ok");

        let decoded = decode_rgba(&out);
        assert_eq!(decoded.dimensions(), (100, 100));

        let center = decoded.get_pixel(50, 50);
        assert!(
            center[0] > 200 && center[1] < 80 && center[2] < 80 && center[3] > 200,
            "expected center area to remain red after cover crop, got {:?}",
            center
        );
    }

    #[test]
    fn all_modes_return_original_when_small_and_upscale_is_false() {
        let src = encode_png(&make_pattern_rgba(100, 50));
        let p = ImageRsProcessor::default();

        for mode in [ResizeMode::Fit, ResizeMode::Contain, ResizeMode::Cover] {
            let out = p
                .resize_same_format(
                    &src,
                    "image/png",
                    ResizeOpts::new(500, 500, false, mode, BgColor::white()),
                )
                .expect("resize ok");

            assert_eq!(
                decode_dims(&out),
                (100, 50),
                "mode {mode} should return original dimensions"
            );
        }
    }

    #[test]
    fn jpeg_output_is_jpeg() {
        let p = ImageRsProcessor::default();
        let src = encode_png(&make_pattern_rgba(300, 200));

        let out = p
            .resize_same_format(
                &src,
                "image/jpeg",
                ResizeOpts::new(100, 100, false, ResizeMode::Fit, BgColor::white()),
            )
            .expect("resize ok");

        assert_jpeg_signature(&out);
    }

    #[test]
    fn png_output_is_png() {
        let p = ImageRsProcessor::default();
        let src = encode_png(&make_pattern_rgba(300, 200));

        let out = p
            .resize_same_format(
                &src,
                "image/png",
                ResizeOpts::new(100, 100, false, ResizeMode::Fit, BgColor::white()),
            )
            .expect("resize ok");

        assert_png_signature(&out);
    }

    #[test]
    fn gif_output_is_gif() {
        let p = ImageRsProcessor::default();
        let src = encode_png(&make_pattern_rgba(300, 200));

        let out = p
            .resize_same_format(
                &src,
                "image/gif",
                ResizeOpts::new(100, 100, false, ResizeMode::Fit, BgColor::white()),
            )
            .expect("resize ok");

        assert_gif_signature(&out);
    }

    #[test]
    fn gif_input_can_be_decoded_and_resized() {
        let p = ImageRsProcessor::default();
        let gif = encode_gif(&make_pattern_rgba(320, 160));

        let out = p
            .resize_same_format(
                &gif,
                "image/png",
                ResizeOpts::new(100, 100, false, ResizeMode::Fit, BgColor::white()),
            )
            .expect("resize ok");

        assert_png_signature(&out);
        let (rw, rh) = decode_dims(&out);
        assert!(rw <= 100 && rh <= 100);
    }

    #[test]
    fn unsupported_content_type_is_rejected() {
        let p = ImageRsProcessor::default();
        let src = encode_png(&make_pattern_rgba(100, 100));

        let err = p
            .resize_same_format(
                &src,
                "image/webp",
                ResizeOpts::new(50, 50, false, ResizeMode::Fit, BgColor::white()),
            )
            .expect_err("must reject unsupported content type");

        assert!(err.to_string().contains("unsupported content-type"));
    }

    #[test]
    fn invalid_image_bytes_are_rejected() {
        let p = ImageRsProcessor::default();

        let err = p
            .resize_same_format(
                b"not an image",
                "image/png",
                ResizeOpts::new(50, 50, false, ResizeMode::Fit, BgColor::white()),
            )
            .expect_err("must reject invalid image bytes");

        let msg = err.to_string();
        assert!(
            msg.contains("read image dimensions")
                || msg.contains("decode image bytes")
                || msg.contains("guess image format from bytes"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn bg_color_to_rgba_maps_channels_exactly() {
        let rgba = bg_color_to_rgba(BgColor::new(1, 2, 3, 4));
        assert_eq!(rgba, Rgba([1, 2, 3, 4]));
    }

    #[test]
    fn encode_same_format_rejects_unsupported_output_format() {
        let img = DynamicImage::ImageRgba8(make_pattern_rgba(10, 10));

        let err = encode_same_format(img, ImageFormat::WebP)
            .expect_err("must reject unsupported output format");

        assert!(err.to_string().contains("unsupported output format"));
    }

    #[test]
    fn processor_rejects_input_when_compressed_bytes_exceed_limit() {
        let p = ImageRsProcessor::new(DecodeLimits::new(10, 10_000, 10_000, 100_000_000));
        let src = encode_png(&make_pattern_rgba(100, 100));

        let err = p
            .resize_same_format(
                &src,
                "image/png",
                ResizeOpts::new(50, 50, false, ResizeMode::Fit, BgColor::white()),
            )
            .expect_err("must reject oversize input bytes");

        assert!(err.to_string().contains("input image too large"));
    }

    #[test]
    fn processor_rejects_input_when_dimensions_exceed_limit() {
        let p = ImageRsProcessor::new(DecodeLimits::new(1024 * 1024, 50, 10_000, 100_000_000));
        let src = encode_png(&make_pattern_rgba(100, 100));

        let err = p
            .resize_same_format(
                &src,
                "image/png",
                ResizeOpts::new(50, 50, false, ResizeMode::Fit, BgColor::white()),
            )
            .expect_err("must reject large width");

        assert!(
            err.to_string().contains("validate image dimensions")
                || err.to_string().contains("image width too large")
        );
    }

    #[test]
    fn processor_rejects_input_when_pixel_count_exceeds_limit() {
        let p = ImageRsProcessor::new(DecodeLimits::new(1024 * 1024, 10_000, 10_000, 5_000));
        let src = encode_png(&make_pattern_rgba(100, 100)); // 10,000 pixels

        let err = p
            .resize_same_format(
                &src,
                "image/png",
                ResizeOpts::new(50, 50, false, ResizeMode::Fit, BgColor::white()),
            )
            .expect_err("must reject large pixel count");

        assert!(
            err.to_string().contains("validate image dimensions")
                || err.to_string().contains("image pixel count too large")
        );
    }

    #[test]
    fn apply_orientation_rotation_6_rotates_clockwise() {
        let src = DynamicImage::ImageRgba8(make_orientation_probe_rgba());
        let out = apply_orientation(src, 6).to_rgba8();

        assert_eq!(out.dimensions(), (2, 3));
        assert_eq!(*out.get_pixel(1, 0), Rgba([255, 0, 0, 255])); // old top-left -> top-right
        assert_eq!(*out.get_pixel(1, 2), Rgba([0, 255, 0, 255])); // old top-right -> bottom-right
        assert_eq!(*out.get_pixel(0, 0), Rgba([0, 0, 255, 255])); // old bottom-left -> top-left
        assert_eq!(*out.get_pixel(0, 2), Rgba([255, 255, 0, 255])); // old bottom-right -> bottom-left
    }

    #[test]
    fn apply_orientation_rotation_3_rotates_180() {
        let src = DynamicImage::ImageRgba8(make_orientation_probe_rgba());
        let out = apply_orientation(src, 3).to_rgba8();

        assert_eq!(out.dimensions(), (3, 2));
        assert_eq!(*out.get_pixel(2, 1), Rgba([255, 0, 0, 255]));
        assert_eq!(*out.get_pixel(0, 1), Rgba([0, 255, 0, 255]));
        assert_eq!(*out.get_pixel(2, 0), Rgba([0, 0, 255, 255]));
        assert_eq!(*out.get_pixel(0, 0), Rgba([255, 255, 0, 255]));
    }

    #[test]
    fn apply_orientation_unknown_value_returns_original() {
        let src = DynamicImage::ImageRgba8(make_orientation_probe_rgba());
        let out = apply_orientation(src.clone(), 999).to_rgba8();
        assert_eq!(out, src.to_rgba8());
    }

    #[test]
    fn maybe_normalize_orientation_is_noop_for_non_jpeg() {
        let src = DynamicImage::ImageRgba8(make_orientation_probe_rgba());
        let out = maybe_normalize_orientation(b"not-exif", "image/png", src.clone()).to_rgba8();
        assert_eq!(out, src.to_rgba8());
    }
}
