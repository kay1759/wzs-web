//! Upload service for storing regular files and processed images.
//!
//! This module provides:
//!
//! - [`MediaDirs`] to configure directory prefixes for images and files
//! - [`UploadImageParams`] and [`UploadImageParamsInput`] for image resize options
//! - [`UploadService`] as the application-facing upload entry point
//! - [`UploadResult`] as the result of a successful upload
//!
//! # Design
//!
//! [`UploadService`] depends on abstractions instead of concrete backends:
//!
//! - [`FileStorage`] for persistence
//! - [`ImageProcessor`] for image resizing
//!
//! This makes the service easy to test by injecting mock implementations.
//!
//! # Behavior
//!
//! - If `image_params` are provided, the upload is treated as an image upload.
//! - If `image_params` are not provided, the upload is stored as a regular file.
//! - Image uploads are resized before saving.
//! - Regular files are stored as-is.
//! - Image uploads are stored under `image_dir/YYYYMM/...`.
//! - Regular files are stored under `file_dir/YYYYMM/...`.

use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use uuid::Uuid;

use super::storage::FileStorage;
use crate::image::processor::{BgColor, ImageProcessor, ResizeMode, ResizeOpts};

/// Directory configuration for uploaded media.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaDirs {
    /// Directory prefix for processed image uploads.
    pub image_dir: String,
    /// Directory prefix for non-processed file uploads.
    pub file_dir: String,
}

impl MediaDirs {
    /// Creates a new directory configuration.
    pub fn new(image_dir: impl Into<String>, file_dir: impl Into<String>) -> Self {
        Self {
            image_dir: image_dir.into(),
            file_dir: file_dir.into(),
        }
    }
}

impl Default for MediaDirs {
    fn default() -> Self {
        Self {
            image_dir: "images".into(),
            file_dir: "files".into(),
        }
    }
}

/// Typed parameters for image uploads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadImageParams {
    /// Target maximum width.
    pub max_width: u32,
    /// Target maximum height.
    pub max_height: u32,
    /// Whether smaller images may be enlarged.
    pub upscale: bool,
    /// Resize strategy.
    pub resize_mode: ResizeMode,
    /// Background color used for contain mode padding.
    pub background: BgColor,
}

impl UploadImageParams {
    /// Converts this value into the backend-agnostic resize options type.
    pub fn to_resize_opts(&self) -> ResizeOpts {
        ResizeOpts::new(
            self.max_width,
            self.max_height,
            self.upscale,
            self.resize_mode,
            self.background,
        )
    }
}

/// Raw image parameter input, typically parsed from HTTP form values.
///
/// All fields are optional here so callers can detect whether image resizing
/// has been requested at all. Once any value is present, all values become
/// required for successful parsing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UploadImageParamsInput {
    /// Raw `maxWidth` input.
    pub max_width: Option<String>,
    /// Raw `maxHeight` input.
    pub max_height: Option<String>,
    /// Raw `upscale` input.
    pub upscale: Option<String>,
    /// Raw `resizeMode` input.
    pub resize_mode: Option<String>,
    /// Raw `background` input.
    pub background: Option<String>,
}

impl UploadImageParamsInput {
    /// Returns `true` if any image-related input field is present.
    pub fn has_any_value(&self) -> bool {
        self.max_width.is_some()
            || self.max_height.is_some()
            || self.upscale.is_some()
            || self.resize_mode.is_some()
            || self.background.is_some()
    }

    /// Parses raw input into typed image parameters.
    ///
    /// # Returns
    ///
    /// - `Ok(None)` if no image parameter fields are present
    /// - `Ok(Some(...))` if all required fields are present and valid
    /// - `Err(...)` if any field is invalid or missing once image resizing is requested
    pub fn parse(self) -> Result<Option<UploadImageParams>> {
        if !self.has_any_value() {
            return Ok(None);
        }

        let max_width = parse_required_u32(self.max_width.as_deref(), "maxWidth")?;
        let max_height = parse_required_u32(self.max_height.as_deref(), "maxHeight")?;
        let upscale = parse_required_bool(self.upscale.as_deref(), "upscale")?;
        let resize_mode = parse_required_resize_mode(self.resize_mode.as_deref(), "resizeMode")?;
        let background = parse_required_bg_color(self.background.as_deref(), "background")?;

        Ok(Some(UploadImageParams {
            max_width,
            max_height,
            upscale,
            resize_mode,
            background,
        }))
    }
}

/// Parses a required `u32` value.
fn parse_required_u32(value: Option<&str>, name: &str) -> Result<u32> {
    let raw = value.ok_or_else(|| anyhow::anyhow!("{name} is required"))?;
    raw.parse::<u32>()
        .with_context(|| format!("invalid {name}: {raw}"))
}

/// Parses a required boolean value.
///
/// Accepted truthy values:
/// - `true`
/// - `1`
/// - `yes`
/// - `on`
///
/// Accepted falsy values:
/// - `false`
/// - `0`
/// - `no`
/// - `off`
fn parse_required_bool(value: Option<&str>, name: &str) -> Result<bool> {
    let raw = value.ok_or_else(|| anyhow::anyhow!("{name} is required"))?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => bail!("invalid {name}: {raw}"),
    }
}

/// Parses a required resize mode value.
fn parse_required_resize_mode(value: Option<&str>, name: &str) -> Result<ResizeMode> {
    let raw = value.ok_or_else(|| anyhow::anyhow!("{name} is required"))?;
    ResizeMode::from_str(raw).with_context(|| format!("invalid {name}: {raw}"))
}

/// Parses a required background color value.
fn parse_required_bg_color(value: Option<&str>, name: &str) -> Result<BgColor> {
    let raw = value.ok_or_else(|| anyhow::anyhow!("{name} is required"))?;
    BgColor::from_str(raw).with_context(|| format!("invalid {name}: {raw}"))
}

/// Successful upload result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadResult {
    /// Storage key used for saving the file.
    pub key: String,
    /// Absolute or backend-resolved stored path.
    pub abs_path: String,
    /// Saved byte size.
    pub bytes: u64,
    /// Final content type recorded for the upload.
    pub content_type: String,
}

/// Service for handling regular file uploads and image uploads.
///
/// This service coordinates:
///
/// - file path generation
/// - filename sanitization
/// - image resizing
/// - file persistence
///
/// Concrete behavior is delegated to injected implementations of
/// [`FileStorage`] and [`ImageProcessor`].
#[derive(Clone)]
pub struct UploadService {
    storage: Arc<dyn FileStorage>,
    image: Arc<dyn ImageProcessor>,
    dirs: MediaDirs,
}

impl UploadService {
    /// Creates a new upload service with default directory names.
    pub fn new(storage: Arc<dyn FileStorage>, image: Arc<dyn ImageProcessor>) -> Self {
        Self {
            storage,
            image,
            dirs: MediaDirs::default(),
        }
    }

    /// Creates a new upload service with custom directory names.
    pub fn with_dirs(
        storage: Arc<dyn FileStorage>,
        image: Arc<dyn ImageProcessor>,
        dirs: MediaDirs,
    ) -> Self {
        Self {
            storage,
            image,
            dirs,
        }
    }

    /// Returns the configured media directories.
    pub fn dirs(&self) -> &MediaDirs {
        &self.dirs
    }

    /// Uploads either a processed image or a regular file.
    ///
    /// If `image_params` is `Some(...)`, the upload is handled as an image upload.
    /// Otherwise it is handled as a regular file upload.
    pub fn upload(
        &self,
        filename: &str,
        content_type: &str,
        bytes: &[u8],
        image_params: Option<UploadImageParams>,
    ) -> Result<UploadResult> {
        match image_params {
            Some(params) => self.upload_image(content_type, bytes, params),
            None => self.upload_file(filename, content_type, bytes),
        }
    }

    /// Uploads and processes an image.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    ///
    /// - the content type is not supported as an image
    /// - image processing fails
    /// - file persistence fails
    fn upload_image(
        &self,
        content_type: &str,
        bytes: &[u8],
        params: UploadImageParams,
    ) -> Result<UploadResult> {
        if !self.image.is_supported(content_type) {
            bail!("content type is not supported as an image: {content_type}");
        }

        let id = Uuid::new_v4().to_string();
        let yyyymm = Utc::now().format("%Y%m").to_string();

        let (ext, norm_ct) = normalize_image_type(content_type);
        let resized = self
            .image
            .resize_same_format(bytes, norm_ct, params.to_resize_opts())
            .with_context(|| format!("process image as {norm_ct}"))?;

        let key = format!("{}/{}/{}.{}", self.dirs.image_dir, yyyymm, id, ext);
        let abs = self.storage.save(&key, &resized)?;

        Ok(UploadResult {
            key,
            abs_path: abs,
            bytes: resized.len() as u64,
            content_type: norm_ct.to_string(),
        })
    }

    /// Uploads a regular file without image processing.
    ///
    /// # Errors
    ///
    /// Returns an error if file persistence fails.
    fn upload_file(
        &self,
        filename: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> Result<UploadResult> {
        let id = Uuid::new_v4().to_string();
        let yyyymm = Utc::now().format("%Y%m").to_string();

        let safe_name = sanitize_filename(filename);
        let final_name = if safe_name.is_empty() {
            format!("{id}.bin")
        } else {
            safe_name
        };

        let key = format!("{}/{}/{}", self.dirs.file_dir, yyyymm, final_name);
        let abs = self.storage.save(&key, bytes)?;

        Ok(UploadResult {
            key,
            abs_path: abs,
            bytes: bytes.len() as u64,
            content_type: content_type.to_string(),
        })
    }
}

/// Normalizes an image content type into `(extension, canonical_content_type)`.
///
/// Unknown values fall back to `("bin", "application/octet-stream")`.
fn normalize_image_type(content_type: &str) -> (&'static str, &'static str) {
    match content_type.to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => ("jpg", "image/jpeg"),
        "image/png" => ("png", "image/png"),
        "image/gif" => ("gif", "image/gif"),
        _ => ("bin", "application/octet-stream"),
    }
}

/// Sanitizes a filename for safe storage.
///
/// This function:
///
/// - trims surrounding whitespace
/// - keeps only the basename
/// - replaces dangerous path or shell characters with `_`
fn sanitize_filename(filename: &str) -> String {
    let trimmed = filename.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let basename = Path::new(trimmed)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    basename
        .chars()
        .map(|c| match c {
            '/' | '\\' | '\0' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{bail, Result};
    use std::sync::Mutex;

    /// A hand-written test double for [`FileStorage`].
    ///
    /// It records all save calls and can be configured to fail.
    #[derive(Default)]
    struct MockStorage {
        calls: Mutex<Vec<(String, Vec<u8>)>>,
        result_path: String,
        fail: bool,
    }

    impl MockStorage {
        /// Creates a new mock storage that returns the given path on success.
        fn new(result_path: &str) -> Self {
            Self {
                calls: Mutex::new(vec![]),
                result_path: result_path.to_string(),
                fail: false,
            }
        }

        /// Configures the mock to fail on every save call.
        fn with_fail(mut self) -> Self {
            self.fail = true;
            self
        }

        /// Returns all recorded calls.
        fn calls(&self) -> Vec<(String, Vec<u8>)> {
            self.calls.lock().expect("lock calls").clone()
        }
    }

    impl FileStorage for MockStorage {
        fn save(&self, rel_path: &str, bytes: &[u8]) -> Result<String> {
            self.calls
                .lock()
                .expect("lock calls")
                .push((rel_path.to_string(), bytes.to_vec()));

            if self.fail {
                bail!("save failed");
            }

            Ok(self.result_path.clone())
        }
    }

    /// A hand-written test double for [`ImageProcessor`].
    ///
    /// It records support checks and resize calls, and can be configured to fail.
    #[derive(Default)]
    struct MockImageProcessor {
        supported: bool,
        support_calls: Mutex<Vec<String>>,
        resize_calls: Mutex<Vec<(Vec<u8>, String, ResizeOpts)>>,
        resize_result: Option<Vec<u8>>,
        fail: bool,
    }

    impl MockImageProcessor {
        /// Creates a new mock image processor.
        fn new(supported: bool, resize_result: Vec<u8>) -> Self {
            Self {
                supported,
                support_calls: Mutex::new(vec![]),
                resize_calls: Mutex::new(vec![]),
                resize_result: Some(resize_result),
                fail: false,
            }
        }

        /// Configures the mock to fail on resize.
        fn with_fail(mut self) -> Self {
            self.fail = true;
            self
        }

        /// Returns all recorded support checks.
        fn support_calls(&self) -> Vec<String> {
            self.support_calls
                .lock()
                .expect("lock support calls")
                .clone()
        }

        /// Returns all recorded resize calls.
        fn resize_calls(&self) -> Vec<(Vec<u8>, String, ResizeOpts)> {
            self.resize_calls.lock().expect("lock resize calls").clone()
        }
    }

    impl ImageProcessor for MockImageProcessor {
        fn is_supported(&self, content_type: &str) -> bool {
            self.support_calls
                .lock()
                .expect("lock support calls")
                .push(content_type.to_string());

            self.supported
        }

        fn resize_same_format(
            &self,
            img_bytes: &[u8],
            content_type: &str,
            opts: ResizeOpts,
        ) -> Result<Vec<u8>> {
            self.resize_calls.lock().expect("lock resize calls").push((
                img_bytes.to_vec(),
                content_type.to_string(),
                opts,
            ));

            if self.fail {
                bail!("resize failed");
            }

            Ok(self
                .resize_result
                .clone()
                .unwrap_or_else(|| img_bytes.to_vec()))
        }
    }

    /// Creates a service with configurable test doubles.
    fn make_service_with(
        storage: Arc<MockStorage>,
        image: Arc<MockImageProcessor>,
    ) -> UploadService {
        UploadService::with_dirs(
            storage,
            image,
            MediaDirs {
                image_dir: "images".into(),
                file_dir: "files".into(),
            },
        )
    }

    #[test]
    fn media_dirs_new_builds_custom_values() {
        let dirs = MediaDirs::new("photo", "docs");
        assert_eq!(dirs.image_dir, "photo");
        assert_eq!(dirs.file_dir, "docs");
    }

    #[test]
    fn media_dirs_default_uses_standard_names() {
        let dirs = MediaDirs::default();
        assert_eq!(dirs.image_dir, "images");
        assert_eq!(dirs.file_dir, "files");
    }

    #[test]
    fn upload_image_params_to_resize_opts_builds_expected_value() {
        let params = UploadImageParams {
            max_width: 800,
            max_height: 600,
            upscale: true,
            resize_mode: ResizeMode::Contain,
            background: BgColor::white(),
        };

        let opts = params.to_resize_opts();

        assert_eq!(
            opts,
            ResizeOpts::new(800, 600, true, ResizeMode::Contain, BgColor::white())
        );
    }

    #[test]
    fn parse_image_params_returns_none_when_no_values_supplied() {
        let input = UploadImageParamsInput::default();
        let parsed = input.parse().expect("parse");
        assert!(parsed.is_none());
    }

    #[test]
    fn parse_image_params_parses_valid_values() {
        let input = UploadImageParamsInput {
            max_width: Some("800".into()),
            max_height: Some("600".into()),
            upscale: Some("true".into()),
            resize_mode: Some("contain".into()),
            background: Some("#ffffffff".into()),
        };

        let parsed = input.parse().expect("parse").expect("some");
        assert_eq!(parsed.max_width, 800);
        assert_eq!(parsed.max_height, 600);
        assert!(parsed.upscale);
        assert_eq!(parsed.resize_mode, ResizeMode::Contain);
        assert_eq!(parsed.background, BgColor::white());
    }

    #[test]
    fn parse_image_params_accepts_flexible_boolean_values() {
        let input = UploadImageParamsInput {
            max_width: Some("320".into()),
            max_height: Some("240".into()),
            upscale: Some("yes".into()),
            resize_mode: Some("fit".into()),
            background: Some("#00000000".into()),
        };

        let parsed = input.parse().expect("parse").expect("some");
        assert!(parsed.upscale);
    }

    #[test]
    fn parse_image_params_rejects_partial_values() {
        let input = UploadImageParamsInput {
            max_width: Some("800".into()),
            ..Default::default()
        };

        let err = input.parse().expect_err("must reject partial params");
        assert!(err.to_string().contains("maxHeight is required"));
    }

    #[test]
    fn parse_image_params_rejects_invalid_u32() {
        let input = UploadImageParamsInput {
            max_width: Some("abc".into()),
            max_height: Some("600".into()),
            upscale: Some("true".into()),
            resize_mode: Some("contain".into()),
            background: Some("#ffffffff".into()),
        };

        let err = input.parse().expect_err("must reject invalid width");
        assert!(err.to_string().contains("invalid maxWidth"));
    }

    #[test]
    fn parse_image_params_rejects_invalid_bool() {
        let input = UploadImageParamsInput {
            max_width: Some("800".into()),
            max_height: Some("600".into()),
            upscale: Some("maybe".into()),
            resize_mode: Some("contain".into()),
            background: Some("#ffffffff".into()),
        };

        let err = input.parse().expect_err("must reject invalid bool");
        assert!(err.to_string().contains("invalid upscale"));
    }

    #[test]
    fn parse_image_params_rejects_invalid_resize_mode() {
        let input = UploadImageParamsInput {
            max_width: Some("800".into()),
            max_height: Some("600".into()),
            upscale: Some("true".into()),
            resize_mode: Some("stretch".into()),
            background: Some("#ffffffff".into()),
        };

        let err = input.parse().expect_err("must reject invalid resize mode");
        assert!(err.to_string().contains("invalid resizeMode"));
    }

    #[test]
    fn parse_image_params_rejects_invalid_background() {
        let input = UploadImageParamsInput {
            max_width: Some("800".into()),
            max_height: Some("600".into()),
            upscale: Some("true".into()),
            resize_mode: Some("contain".into()),
            background: Some("white".into()),
        };

        let err = input.parse().expect_err("must reject invalid background");
        assert!(err.to_string().contains("invalid background"));
    }

    #[test]
    fn upload_service_new_uses_default_dirs() {
        let storage = Arc::new(MockStorage::new("/tmp/unused"));
        let image = Arc::new(MockImageProcessor::new(true, b"processed".to_vec()));

        let svc = UploadService::new(storage, image);

        assert_eq!(svc.dirs(), &MediaDirs::default());
    }

    #[test]
    fn upload_with_image_params_resizes_and_saves_under_image_dir() {
        let storage = Arc::new(MockStorage::new("/tmp/images/saved.png"));
        let image = Arc::new(MockImageProcessor::new(true, b"processed".to_vec()));
        let svc = make_service_with(storage.clone(), image.clone());

        let params = UploadImageParams {
            max_width: 800,
            max_height: 600,
            upscale: true,
            resize_mode: ResizeMode::Contain,
            background: BgColor::white(),
        };

        let out = svc
            .upload("a.png", "image/png", b"raw-image", Some(params.clone()))
            .expect("upload");

        assert!(out.key.starts_with("images/"));
        assert!(out.key.ends_with(".png"));
        assert_eq!(out.abs_path, "/tmp/images/saved.png");
        assert_eq!(out.bytes, 9);
        assert_eq!(out.content_type, "image/png");

        let support_calls = image.support_calls();
        assert_eq!(support_calls, vec!["image/png"]);

        let resize_calls = image.resize_calls();
        assert_eq!(resize_calls.len(), 1);
        assert_eq!(resize_calls[0].0, b"raw-image");
        assert_eq!(resize_calls[0].1, "image/png");
        assert_eq!(resize_calls[0].2, params.to_resize_opts());

        let storage_calls = storage.calls();
        assert_eq!(storage_calls.len(), 1);
        assert!(storage_calls[0].0.starts_with("images/"));
        assert_eq!(storage_calls[0].1, b"processed");
    }

    #[test]
    fn upload_image_normalizes_jpg_content_type() {
        let storage = Arc::new(MockStorage::new("/tmp/images/saved.jpg"));
        let image = Arc::new(MockImageProcessor::new(true, b"processed-jpg".to_vec()));
        let svc = make_service_with(storage.clone(), image.clone());

        let params = UploadImageParams {
            max_width: 100,
            max_height: 100,
            upscale: false,
            resize_mode: ResizeMode::Fit,
            background: BgColor::white(),
        };

        let out = svc
            .upload("a.jpg", "image/jpg", b"raw-jpg", Some(params))
            .expect("upload");

        assert!(out.key.starts_with("images/"));
        assert!(out.key.ends_with(".jpg"));
        assert_eq!(out.content_type, "image/jpeg");

        let resize_calls = image.resize_calls();
        assert_eq!(resize_calls.len(), 1);
        assert_eq!(resize_calls[0].1, "image/jpeg");

        let storage_calls = storage.calls();
        assert_eq!(storage_calls.len(), 1);
        assert_eq!(storage_calls[0].1, b"processed-jpg");
    }

    #[test]
    fn upload_without_image_params_saves_under_file_dir_even_for_images() {
        let storage = Arc::new(MockStorage::new("/tmp/files/photo.png"));
        let image = Arc::new(MockImageProcessor::new(true, b"processed".to_vec()));
        let svc = make_service_with(storage.clone(), image.clone());

        let out = svc
            .upload("photo.png", "image/png", b"raw-image", None)
            .expect("upload");

        assert!(out.key.starts_with("files/"));
        assert!(out.key.ends_with("/photo.png"));
        assert_eq!(out.abs_path, "/tmp/files/photo.png");
        assert_eq!(out.bytes, 9);
        assert_eq!(out.content_type, "image/png");

        let support_calls = image.support_calls();
        assert!(support_calls.is_empty());

        let resize_calls = image.resize_calls();
        assert!(resize_calls.is_empty());

        let storage_calls = storage.calls();
        assert_eq!(storage_calls.len(), 1);
        assert!(storage_calls[0].0.starts_with("files/"));
        assert_eq!(storage_calls[0].1, b"raw-image");
    }

    #[test]
    fn upload_image_rejects_unsupported_content_type() {
        let storage = Arc::new(MockStorage::new("/tmp/unused"));
        let image = Arc::new(MockImageProcessor::new(false, b"processed".to_vec()));
        let svc = make_service_with(storage.clone(), image.clone());

        let params = UploadImageParams {
            max_width: 800,
            max_height: 600,
            upscale: true,
            resize_mode: ResizeMode::Contain,
            background: BgColor::white(),
        };

        let err = svc
            .upload("a.txt", "text/plain", b"hello", Some(params))
            .expect_err("must reject non-image content type");

        assert!(err
            .to_string()
            .contains("content type is not supported as an image"));

        let support_calls = image.support_calls();
        assert_eq!(support_calls, vec!["text/plain"]);

        let resize_calls = image.resize_calls();
        assert!(resize_calls.is_empty());

        let storage_calls = storage.calls();
        assert!(storage_calls.is_empty());
    }

    #[test]
    fn upload_image_returns_error_when_resize_fails() {
        let storage = Arc::new(MockStorage::new("/tmp/unused"));
        let image = Arc::new(MockImageProcessor::new(true, b"processed".to_vec()).with_fail());
        let svc = make_service_with(storage.clone(), image.clone());

        let params = UploadImageParams {
            max_width: 800,
            max_height: 600,
            upscale: true,
            resize_mode: ResizeMode::Contain,
            background: BgColor::white(),
        };

        let err = svc
            .upload("a.png", "image/png", b"raw-image", Some(params))
            .expect_err("resize must fail");

        assert!(err.to_string().contains("process image as image/png"));
        assert!(format!("{err:#}").contains("resize failed"));

        let support_calls = image.support_calls();
        assert_eq!(support_calls, vec!["image/png"]);

        let resize_calls = image.resize_calls();
        assert_eq!(resize_calls.len(), 1);

        let storage_calls = storage.calls();
        assert!(storage_calls.is_empty());
    }

    #[test]
    fn upload_image_returns_error_when_storage_save_fails() {
        let storage = Arc::new(MockStorage::new("/tmp/unused").with_fail());
        let image = Arc::new(MockImageProcessor::new(true, b"processed".to_vec()));
        let svc = make_service_with(storage.clone(), image.clone());

        let params = UploadImageParams {
            max_width: 800,
            max_height: 600,
            upscale: true,
            resize_mode: ResizeMode::Contain,
            background: BgColor::white(),
        };

        let err = svc
            .upload("a.png", "image/png", b"raw-image", Some(params))
            .expect_err("storage save must fail");

        assert!(err.to_string().contains("save failed"));

        let support_calls = image.support_calls();
        assert_eq!(support_calls, vec!["image/png"]);

        let resize_calls = image.resize_calls();
        assert_eq!(resize_calls.len(), 1);

        let storage_calls = storage.calls();
        assert_eq!(storage_calls.len(), 1);
        assert!(storage_calls[0].0.starts_with("images/"));
        assert_eq!(storage_calls[0].1, b"processed");
    }

    #[test]
    fn upload_file_returns_error_when_storage_save_fails() {
        let storage = Arc::new(MockStorage::new("/tmp/unused").with_fail());
        let image = Arc::new(MockImageProcessor::new(true, b"processed".to_vec()));
        let svc = make_service_with(storage.clone(), image.clone());

        let err = svc
            .upload("doc.txt", "text/plain", b"hello", None)
            .expect_err("storage save must fail");

        assert!(err.to_string().contains("save failed"));

        let support_calls = image.support_calls();
        assert!(support_calls.is_empty());

        let resize_calls = image.resize_calls();
        assert!(resize_calls.is_empty());

        let storage_calls = storage.calls();
        assert_eq!(storage_calls.len(), 1);
        assert!(storage_calls[0].0.starts_with("files/"));
        assert_eq!(storage_calls[0].1, b"hello");
    }

    #[test]
    fn upload_file_uses_sanitized_filename() {
        let storage = Arc::new(MockStorage::new("/tmp/files/passwd"));
        let image = Arc::new(MockImageProcessor::new(true, b"processed".to_vec()));
        let svc = make_service_with(storage.clone(), image);

        let out = svc
            .upload("../../etc/passwd", "text/plain", b"hello", None)
            .expect("upload");

        assert!(out.key.starts_with("files/"));
        assert!(out.key.ends_with("/passwd"));

        let storage_calls = storage.calls();
        assert_eq!(storage_calls.len(), 1);
        assert!(storage_calls[0].0.ends_with("/passwd"));
    }

    #[test]
    fn upload_file_generates_bin_name_when_filename_is_empty() {
        let storage = Arc::new(MockStorage::new("/tmp/files/generated.bin"));
        let image = Arc::new(MockImageProcessor::new(true, b"processed".to_vec()));
        let svc = make_service_with(storage.clone(), image);

        let out = svc
            .upload("   ", "application/pdf", b"pdf", None)
            .expect("upload");

        assert!(out.key.starts_with("files/"));
        assert!(out.key.ends_with(".bin"));

        let storage_calls = storage.calls();
        assert_eq!(storage_calls.len(), 1);
        assert!(storage_calls[0].0.ends_with(".bin"));
        assert_eq!(storage_calls[0].1, b"pdf");
    }

    #[test]
    fn sanitize_filename_removes_dangerous_characters() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename(r"..\..\a\b\c.txt"), ".._.._a_b_c.txt");
        assert_eq!(
            sanitize_filename("a:b*c?d\"e<f>g|.txt"),
            "a_b_c_d_e_f_g_.txt"
        );
    }

    #[test]
    fn sanitize_filename_trims_and_handles_empty_values() {
        assert_eq!(sanitize_filename("  hello.txt  "), "hello.txt");
        assert_eq!(sanitize_filename(""), "");
        assert_eq!(sanitize_filename("   "), "");
    }

    #[test]
    fn normalize_image_type_maps_expected_values() {
        assert_eq!(normalize_image_type("image/jpeg"), ("jpg", "image/jpeg"));
        assert_eq!(normalize_image_type("image/jpg"), ("jpg", "image/jpeg"));
        assert_eq!(normalize_image_type("image/png"), ("png", "image/png"));
        assert_eq!(normalize_image_type("image/gif"), ("gif", "image/gif"));
    }

    #[test]
    fn normalize_image_type_is_case_insensitive_and_falls_back_for_unknown_values() {
        assert_eq!(normalize_image_type("IMAGE/PNG"), ("png", "image/png"));
        assert_eq!(
            normalize_image_type("image/webp"),
            ("bin", "application/octet-stream")
        );
    }
}
