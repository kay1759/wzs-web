use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use uuid::Uuid;

use super::storage::FileStorage;
use crate::image::processor::{BgColor, ImageProcessor, ResizeMode, ResizeOpts};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaDirs {
    /// Directory prefix for processed image uploads.
    pub image_dir: String,
    /// Directory prefix for non-processed file uploads.
    pub file_dir: String,
}

impl MediaDirs {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadImageParams {
    pub max_width: u32,
    pub max_height: u32,
    pub upscale: bool,
    pub resize_mode: ResizeMode,
    pub background: BgColor,
}

impl UploadImageParams {
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UploadImageParamsInput {
    pub max_width: Option<String>,
    pub max_height: Option<String>,
    pub upscale: Option<String>,
    pub resize_mode: Option<String>,
    pub background: Option<String>,
}

impl UploadImageParamsInput {
    pub fn has_any_value(&self) -> bool {
        self.max_width.is_some()
            || self.max_height.is_some()
            || self.upscale.is_some()
            || self.resize_mode.is_some()
            || self.background.is_some()
    }

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

fn parse_required_u32(value: Option<&str>, name: &str) -> Result<u32> {
    let raw = value.ok_or_else(|| anyhow::anyhow!("{name} is required"))?;
    raw.parse::<u32>()
        .with_context(|| format!("invalid {name}: {raw}"))
}

fn parse_required_bool(value: Option<&str>, name: &str) -> Result<bool> {
    let raw = value.ok_or_else(|| anyhow::anyhow!("{name} is required"))?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => bail!("invalid {name}: {raw}"),
    }
}

fn parse_required_resize_mode(value: Option<&str>, name: &str) -> Result<ResizeMode> {
    let raw = value.ok_or_else(|| anyhow::anyhow!("{name} is required"))?;
    ResizeMode::from_str(raw).with_context(|| format!("invalid {name}: {raw}"))
}

fn parse_required_bg_color(value: Option<&str>, name: &str) -> Result<BgColor> {
    let raw = value.ok_or_else(|| anyhow::anyhow!("{name} is required"))?;
    BgColor::from_str(raw).with_context(|| format!("invalid {name}: {raw}"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadResult {
    pub key: String,
    pub abs_path: String,
    pub bytes: u64,
    pub content_type: String,
}

#[derive(Clone)]
pub struct UploadService {
    storage: Arc<dyn FileStorage>,
    image: Arc<dyn ImageProcessor>,
    dirs: MediaDirs,
}

impl UploadService {
    pub fn new(storage: Arc<dyn FileStorage>, image: Arc<dyn ImageProcessor>) -> Self {
        Self {
            storage,
            image,
            dirs: MediaDirs::default(),
        }
    }

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

    pub fn dirs(&self) -> &MediaDirs {
        &self.dirs
    }

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

fn normalize_image_type(content_type: &str) -> (&'static str, &'static str) {
    match content_type.to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => ("jpg", "image/jpeg"),
        "image/png" => ("png", "image/png"),
        "image/gif" => ("gif", "image/gif"),
        _ => ("bin", "application/octet-stream"),
    }
}

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
    use anyhow::Result;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MockStorage {
        calls: Mutex<Vec<(String, Vec<u8>)>>,
    }

    impl FileStorage for MockStorage {
        fn save(&self, rel_path: &str, bytes: &[u8]) -> Result<String> {
            self.calls
                .lock()
                .expect("lock calls")
                .push((rel_path.to_string(), bytes.to_vec()));
            Ok(format!("/tmp/{rel_path}"))
        }
    }

    #[derive(Default)]
    struct MockImageProcessor {
        called: Mutex<Vec<(String, ResizeOpts, Vec<u8>)>>,
    }

    impl ImageProcessor for MockImageProcessor {
        fn is_supported(&self, content_type: &str) -> bool {
            matches!(
                content_type.to_ascii_lowercase().as_str(),
                "image/jpeg" | "image/jpg" | "image/png" | "image/gif"
            )
        }

        fn resize_same_format(
            &self,
            img_bytes: &[u8],
            content_type: &str,
            opts: ResizeOpts,
        ) -> Result<Vec<u8>> {
            self.called.lock().expect("lock called").push((
                content_type.to_string(),
                opts,
                img_bytes.to_vec(),
            ));
            Ok(b"processed".to_vec())
        }
    }

    fn make_service() -> (UploadService, Arc<MockStorage>, Arc<MockImageProcessor>) {
        let storage = Arc::new(MockStorage::default());
        let image = Arc::new(MockImageProcessor::default());

        let svc = UploadService::with_dirs(
            storage.clone(),
            image.clone(),
            MediaDirs {
                image_dir: "images".into(),
                file_dir: "files".into(),
            },
        );

        (svc, storage, image)
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
    fn parse_image_params_rejects_partial_values() {
        let input = UploadImageParamsInput {
            max_width: Some("800".into()),
            ..Default::default()
        };

        let err = input.parse().expect_err("must reject partial params");
        assert!(err.to_string().contains("maxHeight is required"));
    }

    #[test]
    fn upload_with_image_params_processes_and_saves_under_image_dir() {
        let (svc, storage, image) = make_service();

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
        assert_eq!(out.bytes, 9);
        assert_eq!(out.content_type, "image/png");

        let storage_calls = storage.calls.lock().expect("lock calls");
        assert_eq!(storage_calls.len(), 1);
        assert!(storage_calls[0].0.starts_with("images/"));
        assert_eq!(storage_calls[0].1, b"processed");

        let image_calls = image.called.lock().expect("lock image calls");
        assert_eq!(image_calls.len(), 1);
        assert_eq!(image_calls[0].0, "image/png");
        assert_eq!(image_calls[0].1, params.to_resize_opts());
        assert_eq!(image_calls[0].2, b"raw-image");
    }

    #[test]
    fn upload_without_image_params_saves_under_file_dir_even_for_images() {
        let (svc, storage, image) = make_service();

        let out = svc
            .upload("photo.png", "image/png", b"raw-image", None)
            .expect("upload");

        assert!(out.key.starts_with("files/"));
        assert_eq!(out.bytes, 9);
        assert_eq!(out.content_type, "image/png");

        let storage_calls = storage.calls.lock().expect("lock calls");
        assert_eq!(storage_calls.len(), 1);
        assert!(storage_calls[0].0.starts_with("files/"));
        assert_eq!(storage_calls[0].1, b"raw-image");

        let image_calls = image.called.lock().expect("lock image calls");
        assert!(image_calls.is_empty());
    }

    #[test]
    fn upload_image_rejects_unsupported_content_type() {
        let (svc, _storage, _image) = make_service();

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
    fn normalize_image_type_maps_expected_values() {
        assert_eq!(normalize_image_type("image/jpeg"), ("jpg", "image/jpeg"));
        assert_eq!(normalize_image_type("image/jpg"), ("jpg", "image/jpeg"));
        assert_eq!(normalize_image_type("image/png"), ("png", "image/png"));
        assert_eq!(normalize_image_type("image/gif"), ("gif", "image/gif"));
    }
}
