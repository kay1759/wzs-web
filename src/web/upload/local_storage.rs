//! # Local File Storage
//!
//! Provides a concrete implementation of the [`FileStorage`] trait that saves files
//! directly to the local filesystem.
//!
//! This module ensures that:
//! - parent directories are automatically created,
//! - relative paths are sanitized (no `..` traversal),
//! - all paths are stored under a configured root directory.
//!
//! Commonly used for local development or single-host deployments.
//!
//! # Example
//! ```rust,no_run
//! use wzs_web::web::upload::storage::{FileStorage, SavedFile};
//! use wzs_web::web::upload::local_storage::LocalFileStorage;
//! use std::path::Path;
//!
//! let storage = LocalFileStorage::new("/tmp/uploads");
//!
//! let abs_path = storage.save("images/avatar.png", b"binary").unwrap();
//! assert!(Path::new(&abs_path).exists());
//!
//! let saved = SavedFile::new(abs_path, "image/png", 6);
//! println!("Saved to {:?}", saved.path);
//! ```

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use super::storage::FileStorage;

/// Stores uploaded files on the local filesystem.
///
/// Files are written under a configurable root directory,
/// ensuring directory creation and path sanitization.
///
/// # Example
/// ```rust,no_run
/// use wzs_web::web::upload::local_storage::LocalFileStorage;
/// use wzs_web::web::upload::storage::FileStorage;
///
/// let storage = LocalFileStorage::new("/tmp/uploads");
/// let abs = storage.save("docs/readme.txt", b"Hello").unwrap();
/// println!("Saved at: {}", abs);
/// ```
#[derive(Clone, Debug)]
pub struct LocalFileStorage {
    /// Root directory where all files are stored.
    root: PathBuf,
}

impl LocalFileStorage {
    /// Creates a new [`LocalFileStorage`] with the given root directory.
    ///
    /// # Example
    /// ```
    /// use wzs_web::web::upload::local_storage::LocalFileStorage;
    ///
    /// let storage = LocalFileStorage::new("/tmp/data");
    /// assert_eq!(storage.root().to_str().unwrap(), "/tmp/data");
    /// ```
    pub fn new<P: Into<PathBuf>>(root: P) -> Self {
        Self { root: root.into() }
    }

    /// Saves a file under the root directory, automatically creating parent directories.
    ///
    /// # Behavior
    /// - Trims leading slashes from `rel_path`
    /// - Replaces `..` with `_` to avoid directory traversal
    /// - Returns the absolute file path as `String`
    ///
    /// # Errors
    /// Returns [`anyhow::Error`] if file writing fails.
    pub fn save_file(&self, rel_path: &str, bytes: &[u8]) -> Result<String> {
        let safe = rel_path.trim_start_matches('/').replace("..", "_");
        let full = self.root.join(&safe);
        if let Some(dir) = full.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&full, bytes).with_context(|| format!("write {:?}", &full))?;
        Ok(full.to_string_lossy().into_owned())
    }

    /// Returns the configured root path.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl FileStorage for LocalFileStorage {
    /// Saves the file by delegating to [`Self::save_file`].
    fn save(&self, rel_path: &str, bytes: &[u8]) -> Result<String> {
        self.save_file(rel_path, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root() -> PathBuf {
        let mut p = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        p.push(format!("local_file_storage-test-{stamp}"));
        p
    }

    #[test]
    fn save_writes_bytes_and_returns_abs_path() -> Result<()> {
        let root = unique_temp_root();
        fs::create_dir_all(&root)?;
        let storage = LocalFileStorage::new(&root);

        let rel = "images/a/b.txt";
        let data = b"hello world";
        let abs = storage.save(rel, data)?;

        assert!(Path::new(&abs).exists());
        let saved = fs::read(&abs)?;
        assert_eq!(saved, data);

        let expected = root.join(rel);
        assert_eq!(Path::new(&abs), expected);

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn save_creates_parent_directories() -> Result<()> {
        let root = unique_temp_root();
        let storage = LocalFileStorage::new(&root);

        let rel = "deep/nested/dir/file.bin";
        let data = [0u8; 3];
        let abs = storage.save(rel, &data)?;

        assert!(Path::new(&abs).exists());
        assert!(root.join("deep/nested/dir").is_dir());

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn sanitize_blocks_parent_segments() -> Result<()> {
        let root = unique_temp_root();
        fs::create_dir_all(&root)?;
        let storage = LocalFileStorage::new(&root);

        let rel = "../secret.txt";
        let abs = storage.save(rel, b"x")?;

        let expected = root.join("_/secret.txt");
        assert_eq!(Path::new(&abs), expected);
        assert!(expected.exists());

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn root_returns_configured_path() {
        let root = unique_temp_root();
        let storage = LocalFileStorage::new(&root);
        assert_eq!(storage.root(), root.as_path());
    }

    #[test]
    fn leading_slash_is_trimmed() -> Result<()> {
        let root = unique_temp_root();
        fs::create_dir_all(&root)?;
        let storage = LocalFileStorage::new(&root);

        let rel = "/top/level.bin";
        let abs = storage.save(rel, b"y")?;

        let expected = root.join("top/level.bin");
        assert_eq!(Path::new(&abs), expected);
        assert!(expected.exists());

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }
}
