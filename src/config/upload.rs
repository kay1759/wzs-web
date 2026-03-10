//! # Upload Configuration
//!
//! Provides configuration parameters for file and image uploads.
//!
//! Defines the root upload directory and separate subdirectories for
//! images and general files.
//!
//! Typically used by file storage or upload service layers
//! (e.g. local filesystem or S3-compatible adapters).
//!
//! # Example
//! ```rust
//! use wzs_web::config::upload::UploadConfig;
//! use std::path::PathBuf;
//!
//! let cfg = UploadConfig {
//!     root: PathBuf::from("/var/www/uploads"),
//!     image_dir: "images".into(),
//!     file_dir: "files".into(),
//! };
//!
//! assert_eq!(cfg.root, PathBuf::from("/var/www/uploads"));
//! assert_eq!(cfg.image_dir, "images");
//! assert_eq!(cfg.file_dir, "files");
//! ```

use std::path::{Path, PathBuf};

/// Configuration for upload directories.
///
/// Defines base and subdirectory paths for storing uploaded files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadConfig {
    /// Root directory where all uploaded content is stored.
    pub root: PathBuf,
    /// Subdirectory for processed image uploads.
    pub image_dir: String,
    /// Subdirectory for non-processed file uploads.
    pub file_dir: String,
}

impl UploadConfig {
    /// Creates a new upload configuration.
    pub fn new(
        root: impl Into<PathBuf>,
        image_dir: impl Into<String>,
        file_dir: impl Into<String>,
    ) -> Self {
        Self {
            root: root.into(),
            image_dir: image_dir.into(),
            file_dir: file_dir.into(),
        }
    }

    /// Returns the upload root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the configured image directory name/prefix.
    pub fn image_dir(&self) -> &str {
        &self.image_dir
    }

    /// Returns the configured file directory name/prefix.
    pub fn file_dir(&self) -> &str {
        &self.file_dir
    }
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::from("./uploads"),
            image_dir: "images".into(),
            file_dir: "files".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn upload_config_holds_values() {
        let cfg = UploadConfig {
            root: PathBuf::from("/tmp/uploads"),
            image_dir: "imgs".into(),
            file_dir: "docs".into(),
        };

        assert_eq!(cfg.root, PathBuf::from("/tmp/uploads"));
        assert_eq!(cfg.image_dir, "imgs");
        assert_eq!(cfg.file_dir, "docs");
    }

    #[test]
    fn upload_config_new_constructs_correctly() {
        let cfg = UploadConfig::new("/var/data/uploads", "images", "files");

        assert_eq!(cfg.root, PathBuf::from("/var/data/uploads"));
        assert_eq!(cfg.image_dir, "images");
        assert_eq!(cfg.file_dir, "files");
    }

    #[test]
    fn upload_config_accessors_work() {
        let cfg = UploadConfig::new("/data/uploads", "img", "file");

        assert_eq!(cfg.root(), Path::new("/data/uploads"));
        assert_eq!(cfg.image_dir(), "img");
        assert_eq!(cfg.file_dir(), "file");
    }

    #[test]
    fn upload_config_default_is_sane() {
        let cfg = UploadConfig::default();

        assert_eq!(cfg.root, PathBuf::from("./uploads"));
        assert_eq!(cfg.image_dir, "images");
        assert_eq!(cfg.file_dir, "files");
    }

    #[test]
    fn upload_config_clone_and_debug() {
        let cfg = UploadConfig {
            root: PathBuf::from("./var/uploads"),
            image_dir: "images".into(),
            file_dir: "files".into(),
        };

        let clone = cfg.clone();
        assert_eq!(cfg, clone);

        let dbg_str = format!("{:?}", cfg);
        assert!(dbg_str.contains("var/uploads"));
        assert!(dbg_str.contains("images"));
        assert!(dbg_str.contains("files"));
    }

    #[test]
    fn upload_config_equality_check() {
        let cfg1 = UploadConfig {
            root: PathBuf::from("/data"),
            image_dir: "img".into(),
            file_dir: "f".into(),
        };
        let cfg2 = UploadConfig {
            root: PathBuf::from("/data"),
            image_dir: "img".into(),
            file_dir: "f".into(),
        };
        let cfg3 = UploadConfig {
            root: PathBuf::from("/data2"),
            image_dir: "imagez".into(),
            file_dir: "filez".into(),
        };

        assert_eq!(cfg1, cfg2);
        assert_ne!(cfg1, cfg3);
    }
}
