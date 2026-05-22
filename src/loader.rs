use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Abstraction for loading file content.
///
/// Implement this trait to provide custom file loading — for example,
/// to serve pre-loaded in-memory content in Wasm environments where
/// direct filesystem access is unavailable.
pub trait FileLoader {
    fn load(&self, path: &str) -> Result<String, LoadError>;
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("File not found: {0}")]
    NotFound(String),
    #[error("IO error loading '{0}': {1}")]
    Io(String, String),
}

/// Loads files from the local filesystem, resolving relative paths against `base_dir`.
pub struct LocalFileLoader {
    base_dir: PathBuf,
}

impl LocalFileLoader {
    /// Create a loader that resolves relative paths against `base_dir`.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self { base_dir: base_dir.into() }
    }
}

impl FileLoader for LocalFileLoader {
    fn load(&self, path: &str) -> Result<String, LoadError> {
        let full = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.base_dir.join(path)
        };
        std::fs::read_to_string(&full).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                LoadError::NotFound(path.to_string())
            } else {
                LoadError::Io(path.to_string(), e.to_string())
            }
        })
    }
}

/// Mock file loader that serves pre-registered in-memory content.
///
/// Use this in tests or Wasm environments where the host injects file
/// content at runtime rather than reading from disk.
#[derive(Default)]
pub struct MockFileLoader {
    files: HashMap<String, String>,
}

impl MockFileLoader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a virtual file with the given path and content.
    pub fn insert(&mut self, path: impl Into<String>, content: impl Into<String>) {
        self.files.insert(path.into(), content.into());
    }
}

impl FileLoader for MockFileLoader {
    fn load(&self, path: &str) -> Result<String, LoadError> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| LoadError::NotFound(path.to_string()))
    }
}
