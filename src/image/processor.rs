//! # Image Processing Abstractions
//!
//! Defines backend-agnostic types and traits for image resizing.
//!
//! This module provides:
//! - [`BgColor`] — background color used when padding an image in `contain` mode.
//! - [`ResizeMode`] — resize strategy (`fit`, `contain`, `cover`).
//! - [`ResizeOpts`] — configuration for resizing.
//! - [`ImageProcessor`] — trait abstraction for concrete image processing backends.
//!
//! # Design Notes
//!
//! - [`ResizeMode::Fit`] preserves aspect ratio and fits the whole image inside the box.
//! - [`ResizeMode::Contain`] preserves aspect ratio, fits inside the box, and pads the
//!   remaining area with [`BgColor`] to produce an exact output box size.
//! - [`ResizeMode::Cover`] preserves aspect ratio, fills the whole box, and crops overflow.
//! - [`BgColor`] accepts `#rrggbb` and `#rrggbbaa` formats.
//!
//! # Example
//!
//! ```rust
//! use std::str::FromStr;
//! use anyhow::Result;
//! use wzs_web::image::processor::{BgColor, ImageProcessor, ResizeMode, ResizeOpts};
//!
//! struct DummyProcessor;
//!
//! impl ImageProcessor for DummyProcessor {
//!     fn is_supported(&self, content_type: &str) -> bool {
//!         content_type.starts_with("image/")
//!     }
//!
//!     fn resize_same_format(
//!         &self,
//!         img_bytes: &[u8],
//!         _content_type: &str,
//!         _opts: ResizeOpts,
//!     ) -> Result<Vec<u8>> {
//!         Ok(img_bytes.to_vec())
//!     }
//! }
//!
//! let opts = ResizeOpts::new(
//!     800,
//!     600,
//!     true,
//!     ResizeMode::Contain,
//!     BgColor::from_str("#ffffffff")?,
//! );
//!
//! let processor = DummyProcessor;
//! assert!(processor.is_supported("image/png"));
//!
//! let result = processor.resize_same_format(b"abc", "image/png", opts)?;
//! assert_eq!(result, b"abc");
//! # Ok::<(), anyhow::Error>(())
//! ```

use std::fmt;
use std::str::FromStr;

use anyhow::{bail, Result};

/// Background color used for padding in [`ResizeMode::Contain`].
///
/// Supported string formats:
/// - `#rrggbb`   → alpha defaults to `0xff`
/// - `#rrggbbaa`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BgColor {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel.
    pub a: u8,
}

impl BgColor {
    /// Creates a new RGBA color.
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Returns opaque white (`#ffffffff`).
    pub const fn white() -> Self {
        Self::new(255, 255, 255, 255)
    }

    /// Returns fully transparent black (`#00000000`).
    pub const fn transparent() -> Self {
        Self::new(0, 0, 0, 0)
    }

    /// Parses a color from `#rrggbb` or `#rrggbbaa`.
    pub fn from_hex(hex: &str) -> Result<Self> {
        hex.parse()
    }

    /// Returns the color as `#rrggbb`.
    pub fn to_hex_rgb(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Returns the color as `#rrggbbaa`.
    pub fn to_hex_rgba(self) -> String {
        format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
    }
}

impl Default for BgColor {
    fn default() -> Self {
        Self::white()
    }
}

impl fmt::Display for BgColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex_rgba())
    }
}

impl FromStr for BgColor {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        if !s.starts_with('#') {
            bail!("color must start with '#': {s}");
        }

        let hex = &s[1..];
        match hex.len() {
            6 => {
                let r = parse_hex_byte(&hex[0..2], s)?;
                let g = parse_hex_byte(&hex[2..4], s)?;
                let b = parse_hex_byte(&hex[4..6], s)?;
                Ok(Self::new(r, g, b, 255))
            }
            8 => {
                let r = parse_hex_byte(&hex[0..2], s)?;
                let g = parse_hex_byte(&hex[2..4], s)?;
                let b = parse_hex_byte(&hex[4..6], s)?;
                let a = parse_hex_byte(&hex[6..8], s)?;
                Ok(Self::new(r, g, b, a))
            }
            _ => bail!("invalid color format: {s} (expected #rrggbb or #rrggbbaa)"),
        }
    }
}

fn parse_hex_byte(hex: &str, original: &str) -> Result<u8> {
    u8::from_str_radix(hex, 16).map_err(|_| anyhow::anyhow!("invalid hex color: {original}"))
}

/// Resize behavior.
///
/// - [`ResizeMode::Fit`]:
///   Preserve aspect ratio and fit the whole image inside the target bounds.
/// - [`ResizeMode::Contain`]:
///   Preserve aspect ratio, fit inside bounds, and pad with background color to
///   produce an exact output box size.
/// - [`ResizeMode::Cover`]:
///   Preserve aspect ratio, fill the whole target box, and crop overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResizeMode {
    /// Keep aspect ratio and fit entirely within bounds.
    Fit,
    /// Keep aspect ratio, fit entirely, and pad remaining area.
    Contain,
    /// Keep aspect ratio, fill entire bounds, cropping overflow.
    Cover,
}

impl ResizeMode {
    /// Returns the canonical lowercase string form.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Fit => "fit",
            Self::Contain => "contain",
            Self::Cover => "cover",
        }
    }
}

impl fmt::Display for ResizeMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ResizeMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "fit" => Ok(Self::Fit),
            "contain" => Ok(Self::Contain),
            "cover" => Ok(Self::Cover),
            _ => bail!("unsupported resize mode: {s}"),
        }
    }
}

/// Options for resizing an image.
///
/// `max_w` and `max_h` define the target box.
/// `upscale` controls whether images already smaller than the target box may be enlarged.
/// `bg_color` is used only for [`ResizeMode::Contain`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResizeOpts {
    /// Target width in pixels.
    pub max_w: u32,
    /// Target height in pixels.
    pub max_h: u32,
    /// Whether small images may be enlarged.
    pub upscale: bool,
    /// Resize strategy.
    pub resize_mode: ResizeMode,
    /// Background color used for padding in contain mode.
    pub bg_color: BgColor,
}

impl ResizeOpts {
    /// Creates a new set of resize options.
    pub const fn new(
        max_w: u32,
        max_h: u32,
        upscale: bool,
        resize_mode: ResizeMode,
        bg_color: BgColor,
    ) -> Self {
        Self {
            max_w,
            max_h,
            upscale,
            resize_mode,
            bg_color,
        }
    }
}

/// Trait defining common image processing behavior.
///
/// Implementors handle format support detection and resizing while preserving
/// the original output format.
pub trait ImageProcessor: Send + Sync {
    /// Returns `true` if the given MIME content type is supported.
    fn is_supported(&self, content_type: &str) -> bool;

    /// Resizes an image while preserving its original format.
    fn resize_same_format(
        &self,
        img_bytes: &[u8],
        content_type: &str,
        opts: ResizeOpts,
    ) -> Result<Vec<u8>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct MockImageProcessor {
        calls: Mutex<Vec<(Vec<u8>, String, ResizeOpts)>>,
    }

    impl ImageProcessor for MockImageProcessor {
        fn is_supported(&self, content_type: &str) -> bool {
            content_type.to_ascii_lowercase().starts_with("image/")
        }

        fn resize_same_format(
            &self,
            img_bytes: &[u8],
            content_type: &str,
            opts: ResizeOpts,
        ) -> Result<Vec<u8>> {
            self.calls.lock().expect("lock calls").push((
                img_bytes.to_vec(),
                content_type.to_string(),
                opts,
            ));
            Ok(img_bytes.to_vec())
        }
    }

    fn assert_send_sync<T: ?Sized + Send + Sync>() {}
    fn assert_clone_copy_eq<T: Clone + Copy + Eq>() {}
    fn assert_hash<T: std::hash::Hash>() {}

    #[test]
    fn bg_color_new_and_named_constructors_work() {
        let c = BgColor::new(1, 2, 3, 4);
        assert_eq!(c.r, 1);
        assert_eq!(c.g, 2);
        assert_eq!(c.b, 3);
        assert_eq!(c.a, 4);

        assert_eq!(BgColor::white(), BgColor::new(255, 255, 255, 255));
        assert_eq!(BgColor::transparent(), BgColor::new(0, 0, 0, 0));
    }

    #[test]
    fn bg_color_default_is_white() {
        assert_eq!(BgColor::default(), BgColor::white());
    }

    #[test]
    fn bg_color_parses_rrggbb() {
        let c = BgColor::from_str("#a1b2c3").expect("parse color");
        assert_eq!(c, BgColor::new(0xa1, 0xb2, 0xc3, 0xff));
    }

    #[test]
    fn bg_color_parses_rrggbbaa() {
        let c = BgColor::from_str("#10203040").expect("parse color");
        assert_eq!(c, BgColor::new(0x10, 0x20, 0x30, 0x40));
    }

    #[test]
    fn bg_color_parses_uppercase_hex() {
        let c = BgColor::from_str("#A0B1C2D3").expect("parse color");
        assert_eq!(c, BgColor::new(0xA0, 0xB1, 0xC2, 0xD3));
    }

    #[test]
    fn bg_color_from_hex_delegates_to_from_str() {
        let c = BgColor::from_hex("#abcdef").expect("parse color");
        assert_eq!(c, BgColor::new(0xab, 0xcd, 0xef, 0xff));
    }

    #[test]
    fn bg_color_display_outputs_rgba_hex() {
        let c = BgColor::new(0x01, 0x23, 0x45, 0x67);
        assert_eq!(c.to_string(), "#01234567");
    }

    #[test]
    fn bg_color_to_hex_helpers_work() {
        let c = BgColor::new(0xde, 0xad, 0xbe, 0xef);
        assert_eq!(c.to_hex_rgb(), "#deadbe");
        assert_eq!(c.to_hex_rgba(), "#deadbeef");
    }

    #[test]
    fn bg_color_rejects_missing_hash() {
        let err = BgColor::from_str("ffffff").expect_err("must reject missing #");
        assert!(err.to_string().contains("must start with '#'"));
    }

    #[test]
    fn bg_color_rejects_invalid_length() {
        for s in ["#", "#1", "#12", "#123", "#12345", "#1234567", "#123456789"] {
            let err = BgColor::from_str(s).expect_err("must reject invalid length");
            assert!(err.to_string().contains("invalid color format"));
        }
    }

    #[test]
    fn bg_color_rejects_non_hex_characters() {
        for s in ["#zzzzzz", "#12xx56", "#123456gg"] {
            let err = BgColor::from_str(s).expect_err("must reject invalid hex");
            assert!(err.to_string().contains("invalid hex color"));
        }
    }

    #[test]
    fn bg_color_traits_are_as_expected() {
        assert_clone_copy_eq::<BgColor>();
        assert_hash::<BgColor>();

        let a = BgColor::new(1, 2, 3, 4);
        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&BgColor::new(1, 2, 3, 4)));
    }

    #[test]
    fn resize_mode_as_str_and_display_are_canonical() {
        assert_eq!(ResizeMode::Fit.as_str(), "fit");
        assert_eq!(ResizeMode::Contain.as_str(), "contain");
        assert_eq!(ResizeMode::Cover.as_str(), "cover");

        assert_eq!(ResizeMode::Fit.to_string(), "fit");
        assert_eq!(ResizeMode::Contain.to_string(), "contain");
        assert_eq!(ResizeMode::Cover.to_string(), "cover");
    }

    #[test]
    fn resize_mode_from_str_is_case_insensitive() {
        assert_eq!(ResizeMode::from_str("fit").unwrap(), ResizeMode::Fit);
        assert_eq!(ResizeMode::from_str("FIT").unwrap(), ResizeMode::Fit);
        assert_eq!(
            ResizeMode::from_str("Contain").unwrap(),
            ResizeMode::Contain
        );
        assert_eq!(ResizeMode::from_str("COVER").unwrap(), ResizeMode::Cover);
    }

    #[test]
    fn resize_mode_rejects_invalid_values() {
        for s in ["", "stretch", " crop ", "fits", "cover "] {
            let err = ResizeMode::from_str(s).expect_err("must reject invalid mode");
            assert!(err.to_string().contains("unsupported resize mode"));
        }
    }

    #[test]
    fn resize_mode_traits_are_as_expected() {
        assert_clone_copy_eq::<ResizeMode>();
        assert_hash::<ResizeMode>();

        let mut set = HashSet::new();
        set.insert(ResizeMode::Contain);
        assert!(set.contains(&ResizeMode::Contain));
    }

    #[test]
    fn resize_opts_new_constructs_correctly() {
        let opts = ResizeOpts::new(
            800,
            600,
            true,
            ResizeMode::Contain,
            BgColor::new(255, 255, 255, 128),
        );

        assert_eq!(opts.max_w, 800);
        assert_eq!(opts.max_h, 600);
        assert!(opts.upscale);
        assert_eq!(opts.resize_mode, ResizeMode::Contain);
        assert_eq!(opts.bg_color, BgColor::new(255, 255, 255, 128));
    }

    #[test]
    fn resize_opts_is_copy_clone_eq_hash() {
        assert_clone_copy_eq::<ResizeOpts>();
        assert_hash::<ResizeOpts>();

        let a = ResizeOpts::new(1, 2, false, ResizeMode::Fit, BgColor::white());
        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&ResizeOpts::new(
            1,
            2,
            false,
            ResizeMode::Fit,
            BgColor::white()
        )));
    }

    #[test]
    fn mock_image_processor_support_detection_and_resize() {
        let mock = Arc::new(MockImageProcessor::default());
        let proc_obj: Arc<dyn ImageProcessor> = mock.clone();

        assert!(proc_obj.is_supported("image/png"));
        assert!(proc_obj.is_supported("IMAGE/JPEG"));
        assert!(!proc_obj.is_supported("text/plain"));
        assert!(!proc_obj.is_supported("application/octet-stream"));

        let input = b"dummy_bytes".to_vec();
        let opts = ResizeOpts::new(
            123,
            456,
            false,
            ResizeMode::Fit,
            BgColor::new(10, 20, 30, 40),
        );

        let out = proc_obj
            .resize_same_format(&input, "image/png", opts)
            .expect("resize ok");

        assert_eq!(out, input);

        let calls = mock.calls.lock().expect("lock calls");
        assert_eq!(calls.len(), 1);

        let (recorded_bytes, recorded_type, recorded_opts) = &calls[0];
        assert_eq!(recorded_bytes, b"dummy_bytes");
        assert_eq!(recorded_type, "image/png");
        assert_eq!(*recorded_opts, opts);
    }

    #[test]
    fn dyn_image_processor_is_send_sync() {
        assert_send_sync::<dyn ImageProcessor>();
    }
}
