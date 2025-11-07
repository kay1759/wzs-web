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
use std::path::PathBuf;

/// Configuration for upload directories.
///
/// Defines base and subdirectory paths for storing uploaded files.
#[derive(Clone, Debug, PartialEq)]
pub struct UploadConfig {
    /// Root directory where all uploaded content is stored.
    pub root: PathBuf,
    /// Subdirectory for image uploads.
    pub image_dir: String,
    /// Subdirectory for non-image file uploads.
    pub file_dir: String,
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
    fn upload_config_clone_and_debug() {
        let cfg = UploadConfig {
            root: PathBuf::from("./var/uploads"),
            image_dir: "images".into(),
            file_dir: "files".into(),
        };

        // Clone が正しく動作すること
        let clone = cfg.clone();
        assert_eq!(cfg, clone);

        // Debug 出力にフィールドが含まれること
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
