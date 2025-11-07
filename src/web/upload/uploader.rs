use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use super::storage::FileStorage;
use crate::image::processor::{ImageProcessor, ResizeOpts};

#[derive(Clone)]
pub struct MediaDirs {
    pub image_dir: String,
    pub file_dir: String,
}

impl Default for MediaDirs {
    fn default() -> Self {
        Self {
            image_dir: "images".into(),
            file_dir: "files".into(),
        }
    }
}

#[derive(Clone)]
pub struct UploadService {
    storage: Arc<dyn FileStorage>,
    image: Arc<dyn ImageProcessor>,
    dirs: MediaDirs,
    resize: ResizeOpts,
}

impl UploadService {
    pub fn new(
        storage: Arc<dyn FileStorage>,
        image: Arc<dyn ImageProcessor>,
        resize: ResizeOpts,
    ) -> Self {
        Self {
            storage,
            image,
            dirs: MediaDirs::default(),
            resize,
        }
    }

    pub fn with_dirs(
        storage: Arc<dyn FileStorage>,
        image: Arc<dyn ImageProcessor>,
        dirs: MediaDirs,
        resize: ResizeOpts,
    ) -> Self {
        Self {
            storage,
            image,
            dirs,
            resize,
        }
    }

    pub fn upload(
        &self,
        filename: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> Result<(String, String, u64, String)> {
        let is_img = self.image.is_supported(content_type);
        let id = Uuid::new_v4().to_string();
        let yyyymm = Utc::now().format("%Y%m").to_string();

        if is_img {
            let (ext, norm_ct) = match content_type.to_ascii_lowercase().as_str() {
                "image/jpeg" | "image/jpg" => ("jpg", "image/jpeg"),
                "image/png" => ("png", "image/png"),
                "image/gif" => ("gif", "image/gif"),
                _ => ("bin", content_type),
            };

            let resized = self.image.resize_same_format(
                bytes,
                norm_ct,
                self.resize.max_w,
                self.resize.max_h,
            )?;

            let key = format!("{}/{}.{}", yyyymm, id, ext);
            let abs = self.storage.save(&key, &resized)?;
            return Ok((key, abs, resized.len() as u64, norm_ct.to_string()));
        }

        let safe = filename.trim().replace('/', "_");
        let name = if safe.is_empty() {
            format!("{id}.bin")
        } else {
            safe
        };
        let key = format!("{}/{}", self.dirs.file_dir, name);
        let abs = self.storage.save(&key, bytes)?;
        Ok((key, abs, bytes.len() as u64, content_type.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{bail, Result};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct StubImageProc {
        calls: Mutex<Vec<(String, u32, u32)>>,
        out: Vec<u8>,
    }

    impl StubImageProc {
        fn with_out(out: &[u8]) -> Self {
            Self {
                calls: Mutex::new(vec![]),
                out: out.to_vec(),
            }
        }
        fn calls(&self) -> Vec<(String, u32, u32)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl ImageProcessor for StubImageProc {
        fn is_supported(&self, content_type: &str) -> bool {
            content_type.to_ascii_lowercase().starts_with("image/")
        }
        fn resize_same_format(
            &self,
            _img_bytes: &[u8],
            content_type: &str,
            max_w: u32,
            max_h: u32,
        ) -> Result<Vec<u8>> {
            self.calls
                .lock()
                .unwrap()
                .push((content_type.to_string(), max_w, max_h));
            Ok(self.out.clone())
        }
    }

    #[derive(Default)]
    struct NeverImageProc;
    impl ImageProcessor for NeverImageProc {
        fn is_supported(&self, _content_type: &str) -> bool {
            false
        }
        fn resize_same_format(
            &self,
            _img_bytes: &[u8],
            _content_type: &str,
            _max_w: u32,
            _max_h: u32,
        ) -> Result<Vec<u8>> {
            bail!("should not be called")
        }
    }

    #[derive(Default)]
    struct StubStorage {
        calls: Mutex<Vec<(String, usize)>>,
    }
    impl StubStorage {
        fn calls(&self) -> Vec<(String, usize)> {
            self.calls.lock().unwrap().clone()
        }
    }
    impl FileStorage for StubStorage {
        fn save(&self, rel_path: &str, bytes: &[u8]) -> Result<String> {
            self.calls
                .lock()
                .unwrap()
                .push((rel_path.to_string(), bytes.len()));
            Ok(format!("/abs/{}", rel_path))
        }
    }

    #[test]
    fn non_image_saved_under_file_dir_and_filename_is_sanitized() {
        let storage_stub = Arc::new(StubStorage::default());
        let storage: Arc<dyn FileStorage> = storage_stub.clone();
        let image: Arc<dyn ImageProcessor> = Arc::new(NeverImageProc::default());

        let uc = UploadService::with_dirs(
            storage.clone(),
            image,
            MediaDirs {
                image_dir: "images".into(),
                file_dir: "files".into(),
            },
            ResizeOpts {
                max_w: 100,
                max_h: 100,
            },
        );

        let (key, _abs, _bytes_saved, _ct) = uc
            .upload("docs/readme.txt", "text/plain", b"hello")
            .unwrap();

        let calls = storage_stub.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, key);
        assert_eq!(calls[0].1, 5);
    }

    #[test]
    fn non_image_empty_filename_defaults_to_uuid_bin() {
        let storage: Arc<dyn FileStorage> = Arc::new(StubStorage::default());
        let image: Arc<dyn ImageProcessor> = Arc::new(NeverImageProc::default());

        let uc = UploadService::new(storage.clone(), image, ResizeOpts { max_w: 1, max_h: 1 });

        let (key, abs, bytes_saved, ct) = uc.upload("", "application/octet-stream", b"x").unwrap();

        assert!(key.starts_with("files/"));
        assert!(key.ends_with(".bin"));
        assert_eq!(abs, format!("/abs/{}", key));
        assert_eq!(bytes_saved, 1);
        assert_eq!(ct, "application/octet-stream");
    }

    #[test]
    fn image_png_resized_and_key_with_yyyymm_and_ext() {
        let storage_stub = Arc::new(StubStorage::default());
        let storage: Arc<dyn FileStorage> = storage_stub.clone();

        let img_stub = Arc::new(StubImageProc::with_out(b"RESIZED"));
        let image: Arc<dyn ImageProcessor> = img_stub.clone();

        let uc = UploadService::new(
            storage.clone(),
            image.clone(),
            ResizeOpts {
                max_w: 640,
                max_h: 480,
            },
        );

        let (_key, _abs, _bytes_saved, _ct) =
            uc.upload("ignored.png", "image/png", b"orig").unwrap();

        // ← 具体型ハンドルから参照
        let calls = img_stub.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "image/png");
        assert_eq!(calls[0].1, 640);
        assert_eq!(calls[0].2, 480);
    }

    #[test]
    fn image_jpeg_and_gif_ext_mapping() {
        let storage: Arc<dyn FileStorage> = Arc::new(StubStorage::default());
        let image: Arc<dyn ImageProcessor> = Arc::new(StubImageProc::with_out(b"X"));

        let uc = UploadService::new(
            storage.clone(),
            image,
            ResizeOpts {
                max_w: 10,
                max_h: 10,
            },
        );

        let (k1, _, _, c1) = uc.upload("a.jpg", "image/jpeg", b"o").unwrap();
        assert!(k1.ends_with(".jpg"));
        assert_eq!(c1, "image/jpeg");

        let (k2, _, _, c2) = uc.upload("b.gif", "image/gif", b"o").unwrap();
        assert!(k2.ends_with(".gif"));
        assert_eq!(c2, "image/gif");
    }

    fn assert_send_sync<T: ?Sized + Send + Sync>() {}
    #[test]
    fn traits_are_send_sync() {
        assert_send_sync::<dyn FileStorage>();
        assert_send_sync::<dyn ImageProcessor>();
    }
}
