//! # File Storage Abstractions
//!
//! Provides a simple interface for saving uploaded files and tracking metadata.
//!
//! This module defines:
//! - [`SavedFile`] — metadata describing a stored file (path, type, size).
//! - [`FileStorage`] — trait abstraction for file-saving backends (e.g. local FS, S3).
//!
//! The trait is intended to be implemented by various storage layers
//! such as `LocalFileStorage`, `S3Storage`, or `InMemoryStorage` for testing.
//!
//! # Example
//! ```rust
//! use wzs_web::web::upload::storage::{SavedFile, FileStorage};
//! use anyhow::Result;
//!
//! struct LocalStorage;
//!
//! impl FileStorage for LocalStorage {
//!     fn save(&self, rel_path: &str, bytes: &[u8]) -> Result<String> {
//!         let tmp = std::env::temp_dir().join(rel_path);
//!         std::fs::create_dir_all(tmp.parent().unwrap())?;
//!         std::fs::write(&tmp, bytes)?;
//!         Ok(tmp.to_string_lossy().into_owned())
//!     }
//! }
//!
//! let storage = LocalStorage;
//! let path = storage.save("hello.txt", b"hello").unwrap();
//! let saved = SavedFile::new(path.clone(), "text/plain", 5);
//!
//! assert!(path.contains("hello.txt"));
//! assert_eq!(saved.content_type, "text/plain");
//! ```

use anyhow::Result;

/// Metadata for a saved file.
///
/// Holds the file path, MIME type, and size (in bytes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedFile {
    /// Path to the stored file (relative or absolute).
    pub path: String,
    /// MIME content type (e.g. `"image/png"`, `"text/plain"`).
    pub content_type: String,
    /// File size in bytes.
    pub bytes: u64,
}

impl SavedFile {
    /// Creates a new [`SavedFile`] metadata record.
    ///
    /// # Example
    /// ```
    /// use wzs_web::web::upload::storage::SavedFile;
    ///
    /// let file = SavedFile::new("uploads/a.txt", "text/plain", 123);
    /// assert_eq!(file.bytes, 123);
    /// ```
    pub fn new(path: impl Into<String>, content_type: impl Into<String>, bytes: u64) -> Self {
        Self {
            path: path.into(),
            content_type: content_type.into(),
            bytes,
        }
    }
}

/// A trait defining a generic file storage backend.
///
/// Implementors are responsible for saving file data and returning
/// the final path or identifier.
/// Typical implementations include:
/// - Local filesystem storage
/// - Cloud-based storage (e.g. AWS S3, Google Cloud Storage)
/// - In-memory mock storage for tests
pub trait FileStorage: Send + Sync {
    /// Saves a file to the given relative path.
    ///
    /// # Arguments
    /// - `rel_path` — relative destination path (e.g. `"images/123.png"`)
    /// - `bytes` — file contents
    ///
    /// # Returns
    /// The full or relative path of the saved file.
    ///
    /// # Errors
    /// Returns an [`anyhow::Error`] if saving fails.
    fn save(&self, rel_path: &str, bytes: &[u8]) -> Result<String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{bail, Result};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct MockStorage {
        root: String,
        calls: Mutex<Vec<(String, usize)>>,
        fail_on_empty: bool,
    }

    impl MockStorage {
        fn new(root: &str) -> Self {
            Self {
                root: root.to_string(),
                calls: Mutex::new(vec![]),
                fail_on_empty: false,
            }
        }
        fn with_fail_on_empty(mut self) -> Self {
            self.fail_on_empty = true;
            self
        }
        fn calls(&self) -> Vec<(String, usize)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl FileStorage for MockStorage {
        fn save(&self, rel_path: &str, bytes: &[u8]) -> Result<String> {
            if self.fail_on_empty && rel_path.is_empty() {
                bail!("empty rel_path");
            }
            self.calls
                .lock()
                .unwrap()
                .push((rel_path.to_string(), bytes.len()));
            Ok(format!(
                "{}/{}",
                self.root.trim_end_matches('/'),
                rel_path.trim_start_matches('/')
            ))
        }
    }

    #[test]
    fn saved_file_new_builds() {
        let sf = SavedFile::new("p", "text/plain", 3);
        assert_eq!(sf.path, "p");
        assert_eq!(sf.content_type, "text/plain");
        assert_eq!(sf.bytes, 3);

        let sf2 = sf.clone();
        assert_eq!(sf, sf2);
    }

    #[test]
    fn filestorage_save_records_and_returns_path() {
        let storage = Arc::new(MockStorage::new("/abs"));
        let res = storage.save("files/a.txt", b"hello").expect("should save");
        assert_eq!(res, "/abs/files/a.txt");

        let calls = storage.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "files/a.txt");
        assert_eq!(calls[0].1, 5);
    }

    #[test]
    fn filestorage_save_error_on_empty_path_when_enabled() {
        let storage = MockStorage::new("/root").with_fail_on_empty();
        let err = storage.save("", b"abc").unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.to_lowercase().contains("empty rel_path"));
    }

    fn assert_send_sync<T: ?Sized + Send + Sync>() {}
    #[test]
    fn dyn_filestorage_is_send_sync() {
        assert_send_sync::<dyn FileStorage>();
    }
}
