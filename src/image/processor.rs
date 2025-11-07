//! # Image Processing Abstractions
//!
//! Defines a generic interface for image resizing operations and resize options.
//!
//! This module provides:
//! - [`ResizeOpts`] — configuration options for image resizing (max width/height).
//! - [`ImageProcessor`] — a trait abstraction that allows different
//!   image processing backends (e.g. `image-rs`, `magick-rs`, etc.).
//!
//! It enables backend-agnostic implementations, so you can plug in different
//! image libraries while keeping a consistent API across your application.
//!
//! # Example
//! ```rust
//! use wzs_web::image::processor::{ResizeOpts, ImageProcessor};
//! use anyhow::Result;
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
//!         _max_w: u32,
//!         _max_h: u32,
//!     ) -> Result<Vec<u8>> {
//!         Ok(img_bytes.to_vec())
//!     }
//! }
//!
//! let opts = ResizeOpts::new(800, 600);
//! let processor = DummyProcessor;
//!
//! assert!(processor.is_supported("image/png"));
//! let result = processor.resize_same_format(b"abc", "image/png", opts.max_w, opts.max_h).unwrap();
//! assert_eq!(result, b"abc");
//! ```

use anyhow::Result;

/// Options for resizing an image.
///
/// Contains maximum width and height constraints (in pixels).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResizeOpts {
    /// Maximum width (in pixels).
    pub max_w: u32,
    /// Maximum height (in pixels).
    pub max_h: u32,
}

impl ResizeOpts {
    /// Creates a new [`ResizeOpts`] with the specified dimensions.
    pub fn new(max_w: u32, max_h: u32) -> Self {
        Self { max_w, max_h }
    }
}

/// Trait defining common image processing behavior.
///
/// Implementors handle image resizing and format support detection.
/// This allows flexible backend implementations (e.g. using `image` crate, `imageproc`, or native bindings).
pub trait ImageProcessor: Send + Sync {
    /// Returns `true` if the given MIME content type is supported.
    fn is_supported(&self, content_type: &str) -> bool;

    /// Resizes an image while preserving its original format.
    ///
    /// # Arguments
    /// - `img_bytes`: Raw image data.
    /// - `content_type`: MIME type (e.g. `"image/png"`).
    /// - `max_w` / `max_h`: Maximum allowed dimensions.
    ///
    /// # Returns
    /// A resized image as a byte vector, or an error if processing fails.
    fn resize_same_format(
        &self,
        img_bytes: &[u8],
        content_type: &str,
        max_w: u32,
        max_h: u32,
    ) -> Result<Vec<u8>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Mock implementation for testing trait behavior.
    #[derive(Default)]
    struct MockImageProcessor {
        calls: Mutex<Vec<(String, u32, u32)>>,
    }

    impl ImageProcessor for MockImageProcessor {
        fn is_supported(&self, content_type: &str) -> bool {
            content_type.to_ascii_lowercase().starts_with("image/")
        }

        fn resize_same_format(
            &self,
            img_bytes: &[u8],
            content_type: &str,
            max_w: u32,
            max_h: u32,
        ) -> Result<Vec<u8>> {
            self.calls
                .lock()
                .unwrap()
                .push((content_type.to_string(), max_w, max_h));
            Ok(img_bytes.to_vec())
        }
    }

    /// Ensures ResizeOpts correctly stores values.
    #[test]
    fn resize_opts_new_constructs_correctly() {
        let o = ResizeOpts::new(800, 600);
        assert_eq!(o.max_w, 800);
        assert_eq!(o.max_h, 600);

        let o2 = o;
        assert_eq!(o, o2);
        let o3 = o2.clone();
        assert_eq!(o2, o3);
    }

    /// Confirms ImageProcessor correctly detects supported types and resizes.
    #[test]
    fn mock_image_processor_support_detection_and_resize() {
        let mock = Arc::new(MockImageProcessor::default());
        let proc_obj: Arc<dyn ImageProcessor> = mock.clone();

        assert!(proc_obj.is_supported("image/png"));
        assert!(proc_obj.is_supported("IMAGE/JPEG"));
        assert!(!proc_obj.is_supported("text/plain"));

        let input = b"dummy_bytes".to_vec();
        let out = proc_obj
            .resize_same_format(&input, "image/png", 123, 456)
            .expect("resize ok");
        assert_eq!(out, input);

        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "image/png");
        assert_eq!(calls[0].1, 123);
        assert_eq!(calls[0].2, 456);
    }

    /// Ensures the trait object is Send + Sync.
    fn assert_send_sync<T: ?Sized + Send + Sync>() {}
    #[test]
    fn dyn_image_processor_is_send_sync() {
        assert_send_sync::<dyn ImageProcessor>();
    }
}
