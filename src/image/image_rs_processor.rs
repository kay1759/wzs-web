//! # Image Processor Implementation (image-rs)
//!
//! Provides an [`ImageProcessor`] implementation using the [`image`] crate.
//!
//! This processor supports **JPEG**, **PNG**, and **GIF** formats, and performs
//! resizing operations while preserving the original image format.
//!
//! It is suitable for local file upload handling or backend media transformations.
//!
//! # Supported Content Types
//! - `image/jpeg`
//! - `image/jpg`
//! - `image/png`
//! - `image/gif`
//!
//! # Example
//! ```rust,no_run
//! use wzs_web::image::image_rs_processor::ImageRsProcessor;
//! use wzs_web::image::processor::ImageProcessor;
//!
//! let processor = ImageRsProcessor::default();
//! let img_data = std::fs::read("input.png").unwrap();
//!
//! if processor.is_supported("image/png") {
//!     let resized = processor
//!         .resize_same_format(&img_data, "image/png", 800, 600)
//!         .expect("resize ok");
//!     std::fs::write("resized.png", resized).unwrap();
//! }
//! ```
//!
//! # Errors
//! Returns an [`anyhow::Error`] if:
//! - the content type is unsupported,
//! - the image format cannot be guessed or decoded,
//! - writing the resized image fails.

use std::io::Cursor;

use anyhow::{bail, Context, Result};
use image::{
    imageops::FilterType, ColorType, DynamicImage, GenericImageView, ImageFormat, ImageReader,
};

use super::processor::ImageProcessor;

/// A concrete implementation of [`ImageProcessor`] using the `image` crate.
///
/// Supports `JPEG`, `PNG`, and `GIF` images with resizing and same-format encoding.
#[derive(Clone, Debug, Default)]
pub struct ImageRsProcessor;

impl ImageRsProcessor {
    /// Returns `true` if the given MIME type is supported.
    pub fn is_supported(&self, content_type: &str) -> bool {
        matches!(
            content_type.to_ascii_lowercase().as_str(),
            "image/gif" | "image/jpeg" | "image/jpg" | "image/png"
        )
    }

    /// Resizes an image and re-encodes it in the same format.
    ///
    /// Automatically maintains aspect ratio and avoids upscaling smaller images.
    pub fn resize_same_format(
        &self,
        img_bytes: &[u8],
        content_type: &str,
        max_w: u32,
        max_h: u32,
    ) -> Result<Vec<u8>> {
        let img = ImageReader::new(Cursor::new(img_bytes))
            .with_guessed_format()
            .context("guess format")?
            .decode()?;

        let resized = resize_fit(img, max_w, max_h);

        let fmt = match content_type.to_ascii_lowercase().as_str() {
            "image/jpeg" | "image/jpg" => ImageFormat::Jpeg,
            "image/png" => ImageFormat::Png,
            "image/gif" => ImageFormat::Gif,
            _ => bail!("unsupported content-type: {content_type}"),
        };

        let (w, h) = resized.dimensions();
        let mut out = Vec::new();
        let mut cur = Cursor::new(&mut out);

        match fmt {
            ImageFormat::Jpeg => {
                let rgb = resized.to_rgb8();
                image::write_buffer_with_format(
                    &mut cur,
                    &rgb,
                    w,
                    h,
                    ColorType::Rgb8,
                    ImageFormat::Jpeg,
                )?;
            }
            ImageFormat::Png => {
                let rgba = resized.to_rgba8();
                image::write_buffer_with_format(
                    &mut cur,
                    &rgba,
                    w,
                    h,
                    ColorType::Rgba8,
                    ImageFormat::Png,
                )?;
            }
            ImageFormat::Gif => {
                let rgba = resized.to_rgba8();
                image::DynamicImage::ImageRgba8(rgba).write_to(&mut cur, ImageFormat::Gif)?;
            }
            _ => unreachable!(),
        }

        Ok(out)
    }
}

impl ImageProcessor for ImageRsProcessor {
    fn is_supported(&self, content_type: &str) -> bool {
        ImageRsProcessor::is_supported(self, content_type)
    }
    fn resize_same_format(
        &self,
        img_bytes: &[u8],
        content_type: &str,
        max_w: u32,
        max_h: u32,
    ) -> Result<Vec<u8>> {
        ImageRsProcessor::resize_same_format(self, img_bytes, content_type, max_w, max_h)
    }
}

/// Resizes the image proportionally to fit within the specified bounds.
///
/// Uses [`FilterType::Triangle`] for quality-speed balance.
fn resize_fit(img: DynamicImage, max_w: u32, max_h: u32) -> DynamicImage {
    let (w, h) = img.dimensions();
    if w <= max_w && h <= max_h {
        return img;
    }
    img.resize(max_w, max_h, FilterType::Triangle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};
    use std::io::Cursor;

    fn make_png(w: u32, h: u32) -> Vec<u8> {
        let img: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_fn(w, h, |x, y| {
            if (x + y) % 2 == 0 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 255, 0, 255])
            }
        });
        let mut cur = Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut cur,
            img.as_raw(),
            w,
            h,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .expect("encode png");
        cur.into_inner()
    }

    #[test]
    fn supports_expected_mimes() {
        let p = ImageRsProcessor::default();
        assert!(p.is_supported("image/png"));
        assert!(p.is_supported("image/jpeg"));
        assert!(p.is_supported("image/jpg"));
        assert!(p.is_supported("image/gif"));
        assert!(!p.is_supported("text/plain"));
        assert!(!p.is_supported("application/octet-stream"));
        assert!(!p.is_supported("image/webp"));
    }

    #[test]
    fn resize_outputs_jpeg_and_within_bounds() {
        let p = ImageRsProcessor::default();

        let w = 2000;
        let h = 1000;
        let png_bytes = make_png(w, h);

        let out = p
            .resize_same_format(&png_bytes, "image/jpeg", 1280, 1280)
            .expect("resize ok");

        assert!(out.len() >= 3);
        assert_eq!(out[0], 0xFF);
        assert_eq!(out[1], 0xD8);
        assert_eq!(out[2], 0xFF);

        let decoded = image::load_from_memory(&out).expect("decode jpeg");
        let (rw, rh) = decoded.dimensions();
        assert!(rw <= 1280 && rh <= 1280, "resized dims: {rw}x{rh}");
        let ratio = (rw as f64) / (rh as f64);
        assert!((ratio - 2.0).abs() < 0.05, "ratio approx 2.0, got {ratio}");
    }

    #[test]
    fn small_image_is_not_upscaled() {
        let p = ImageRsProcessor::default();
        let png = make_png(100, 50);

        let out = p
            .resize_same_format(&png, "image/jpeg", 500, 500)
            .expect("resize ok");
        let decoded = image::load_from_memory(&out).expect("decode jpeg");
        let (rw, rh) = decoded.dimensions();

        assert_eq!((rw, rh), (100, 50));
    }
}
