//! # Upload Configuration
//!
//! Provides configuration parameters for file and image uploads.
//!
//! Defines the root upload directory and separate subdirectories for
//! images and general files.
//!
//! Configuration can be built directly or from an [`EnvConfig`] environment
//! snapshot.
//!
//! # Environment Variables
//!
//! - `UPLOAD_ROOT` — root upload directory (default: `"./var/uploads"`)
//! - `UPLOAD_IMAGE_DIR` — image subdirectory (default: `"images"`)
//! - `UPLOAD_FILE_DIR` — general file subdirectory (default: `"files"`)
//!
//! # Example
//!
//! ```rust
//! use std::path::PathBuf;
//!
//! use wzs_web::config::env::EnvConfig;
//! use wzs_web::config::upload::UploadConfig;
//!
//! let env = EnvConfig::from_iter([
//!     ("UPLOAD_ROOT", "/var/www/uploads"),
//!     ("UPLOAD_IMAGE_DIR", "images"),
//!     ("UPLOAD_FILE_DIR", "files"),
//! ]);
//!
//! let cfg = UploadConfig::from_env_config(&env);
//!
//! assert_eq!(cfg.root, PathBuf::from("/var/www/uploads"));
//! assert_eq!(cfg.image_dir, "images");
//! assert_eq!(cfg.file_dir, "files");
//! ```

use std::path::{Path, PathBuf};

use crate::config::env::EnvConfig;

/// Default upload root used by environment-based configuration.
const DEFAULT_ENV_UPLOAD_ROOT: &str = "./var/uploads";

/// Default image upload subdirectory.
const DEFAULT_IMAGE_DIR: &str = "images";

/// Default general file upload subdirectory.
const DEFAULT_FILE_DIR: &str = "files";

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

    /// Builds an [`UploadConfig`] from an [`EnvConfig`] snapshot.
    ///
    /// Missing values use the same defaults currently used by
    /// application environment loading:
    ///
    /// - `UPLOAD_ROOT`: `"./var/uploads"`
    /// - `UPLOAD_IMAGE_DIR`: `"images"`
    /// - `UPLOAD_FILE_DIR`: `"files"`
    ///
    /// Note that the environment-based root default differs from
    /// [`UploadConfig::default`], whose existing root is `"./uploads"`.
    /// This distinction is retained for backward compatibility.
    pub fn from_env_config(env: &EnvConfig) -> Self {
        let root = env
            .get("UPLOAD_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_ENV_UPLOAD_ROOT));

        let image_dir = env
            .get_string("UPLOAD_IMAGE_DIR")
            .unwrap_or_else(|| DEFAULT_IMAGE_DIR.to_string());

        let file_dir = env
            .get_string("UPLOAD_FILE_DIR")
            .unwrap_or_else(|| DEFAULT_FILE_DIR.to_string());

        Self {
            root,
            image_dir,
            file_dir,
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
    /// Creates the existing standalone default upload configuration.
    ///
    /// This retains `"./uploads"` as the root for backward compatibility.
    ///
    /// Environment-based configuration uses `"./var/uploads"` instead;
    /// see [`UploadConfig::from_env_config`].
    fn default() -> Self {
        Self {
            root: PathBuf::from("./uploads"),
            image_dir: DEFAULT_IMAGE_DIR.into(),
            file_dir: DEFAULT_FILE_DIR.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn from_env_config_uses_environment_defaults() {
        let env = EnvConfig::default();

        let cfg = UploadConfig::from_env_config(&env);

        assert_eq!(cfg.root, PathBuf::from("./var/uploads"));

        assert_eq!(cfg.image_dir, "images");
        assert_eq!(cfg.file_dir, "files");
    }

    #[test]
    fn from_env_config_reads_values() {
        let env = EnvConfig::from_iter([
            ("UPLOAD_ROOT", "/data/uploads"),
            ("UPLOAD_IMAGE_DIR", "pics"),
            ("UPLOAD_FILE_DIR", "docs"),
        ]);

        let cfg = UploadConfig::from_env_config(&env);

        assert_eq!(cfg.root, PathBuf::from("/data/uploads"));

        assert_eq!(cfg.image_dir, "pics");
        assert_eq!(cfg.file_dir, "docs");
    }

    #[test]
    fn from_env_config_allows_independent_values() {
        let env = EnvConfig::from_iter([("UPLOAD_IMAGE_DIR", "pictures")]);

        let cfg = UploadConfig::from_env_config(&env);

        assert_eq!(cfg.root, PathBuf::from("./var/uploads"));

        assert_eq!(cfg.image_dir, "pictures");
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

        // Preserve the existing standalone Default implementation.
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

        let debug = format!("{cfg:?}");

        assert!(debug.contains("var/uploads"));
        assert!(debug.contains("images"));
        assert!(debug.contains("files"));
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
