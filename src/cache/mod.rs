//! Application-wide cache.
//!
//! The cache is a plain string-keyed map backed by a single JSON file in the
//! user's cache directory. Entries live in memory for the lifetime of the
//! process and are written back when the cache is flushed or dropped.
//!
//! Callers choose the keys. Use namespaced keys such as `author:<uuid>`,
//! `book:<uuid>`, or `search:<normalized query>` so the file stays easy to
//! inspect.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const CACHE_VERSION: u8 = 1;
const CACHE_DIR: &str = "bookshelfman";
const CACHE_FILE: &str = "cache.json";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not determine the user cache directory")]
    NoCacheDir,

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Serialize, Deserialize)]
struct CacheFile {
    #[serde(default)]
    version: u8,
    #[serde(default)]
    entries: HashMap<String, Value>,
}

/// A centralized cache shared by the application.
#[derive(Debug)]
pub struct Cache {
    path: PathBuf,
    entries: RwLock<HashMap<String, Value>>,
    dirty: AtomicBool,
}

impl Cache {
    /// Opens the cache in the platform's user cache directory.
    pub fn open() -> Result<Self> {
        let dir = dirs::cache_dir().ok_or(Error::NoCacheDir)?;

        Self::open_at(dir.join(CACHE_DIR).join(CACHE_FILE))
    }

    /// Opens a cache stored at a specific path.
    ///
    /// A missing file, invalid JSON, or an unknown cache version is treated as
    /// an empty cache. The cache is disposable and can always be rebuilt.
    pub fn open_at(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();

        let entries = match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<CacheFile>(&bytes) {
                Ok(file) if file.version == CACHE_VERSION => file.entries,
                Ok(_) | Err(_) => HashMap::new(),
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => HashMap::new(),
            Err(err) => return Err(err.into()),
        };

        Ok(Self {
            path,
            entries: RwLock::new(entries),
            dirty: AtomicBool::new(false),
        })
    }

    /// Returns the path of the JSON file backing this cache.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Looks up a value and deserializes it.
    ///
    /// A missing key or a value of a different type returns `None`.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let value = {
            let entries = self.entries.read().unwrap_or_else(|err| err.into_inner());
            entries.get(key)?.clone()
        };

        serde_json::from_value(value).ok()
    }

    /// Inserts or replaces a value.
    pub fn set<T: Serialize>(&self, key: impl Into<String>, value: &T) -> Result<()> {
        let value = serde_json::to_value(value)?;
        let mut entries = self.entries.write().unwrap_or_else(|err| err.into_inner());

        entries.insert(key.into(), value);
        self.dirty.store(true, Ordering::Relaxed);

        Ok(())
    }

    /// Removes a value, returning whether it existed.
    pub fn remove(&self, key: &str) -> bool {
        let mut entries = self.entries.write().unwrap_or_else(|err| err.into_inner());
        let removed = entries.remove(key).is_some();

        if removed {
            self.dirty.store(true, Ordering::Relaxed);
        }

        removed
    }

    /// Removes every entry.
    pub fn clear(&self) {
        let mut entries = self.entries.write().unwrap_or_else(|err| err.into_inner());

        if !entries.is_empty() {
            entries.clear();
            self.dirty.store(true, Ordering::Relaxed);
        }
    }

    pub fn len(&self) -> usize {
        self.entries
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Writes the current entries to disk if they have changed.
    ///
    /// The snapshot is written to a temporary file in the same directory and
    /// renamed into place, so readers see either the old or the new file.
    pub fn flush(&self) -> Result<()> {
        if !self.dirty.swap(false, Ordering::Relaxed) {
            return Ok(());
        }

        let snapshot = {
            let entries = self.entries.read().unwrap_or_else(|err| err.into_inner());

            CacheFile {
                version: CACHE_VERSION,
                entries: entries.clone(),
            }
        };

        let parent = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));

        let tmp_path = self
            .path
            .with_extension(format!("json.tmp.{}", std::process::id()));

        let result = (|| -> Result<()> {
            fs::create_dir_all(parent)?;

            let file = File::create(&tmp_path)?;
            serde_json::to_writer_pretty(&file, &snapshot)?;
            file.sync_all()?;

            drop(file);
            fs::rename(&tmp_path, &self.path)?;

            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_file(&tmp_path);
            // Keep the cache dirty so a later flush can retry.
            self.dirty.store(true, Ordering::Relaxed);
        }

        result
    }
}

impl Drop for Cache {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("bookshelfman-cache-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();

            Self(path)
        }

        fn cache_path(&self) -> PathBuf {
            self.0.join(CACHE_FILE)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn stores_and_retrieves_values() {
        let dir = TempDir::new();
        let cache = Cache::open_at(dir.cache_path()).unwrap();

        assert!(cache.is_empty());
        cache
            .set(
                "author:1",
                &serde_json::json!({"name": "Ursula K. Le Guin"}),
            )
            .unwrap();

        assert_eq!(cache.len(), 1);

        let value: Value = cache.get("author:1").unwrap();
        assert_eq!(value["name"], "Ursula K. Le Guin");
    }

    #[test]
    fn persists_to_json() {
        let dir = TempDir::new();

        {
            let cache = Cache::open_at(dir.cache_path()).unwrap();
            cache
                .set("book:1", &serde_json::json!({"title": "The Dispossessed"}))
                .unwrap();
            cache.flush().unwrap();
        }

        let cache = Cache::open_at(dir.cache_path()).unwrap();
        let value: Value = cache.get("book:1").unwrap();

        assert_eq!(value["title"], "The Dispossessed");
    }

    #[test]
    fn flushes_on_drop() {
        let dir = TempDir::new();

        {
            let cache = Cache::open_at(dir.cache_path()).unwrap();
            cache
                .set(
                    "author:2",
                    &serde_json::json!({"name": "Octavia E. Butler"}),
                )
                .unwrap();
        }

        let cache = Cache::open_at(dir.cache_path()).unwrap();

        assert!(cache.get::<Value>("author:2").is_some());
    }

    #[test]
    fn removes_entries() {
        let dir = TempDir::new();
        let cache = Cache::open_at(dir.cache_path()).unwrap();

        cache
            .set("author:3", &serde_json::json!({"name": "N. K. Jemisin"}))
            .unwrap();

        assert!(cache.remove("author:3"));
        assert!(!cache.remove("author:3"));
        assert!(cache.is_empty());
    }

    #[test]
    fn ignores_corrupt_cache_files() {
        let dir = TempDir::new();
        fs::write(dir.cache_path(), b"not json").unwrap();

        let cache = Cache::open_at(dir.cache_path()).unwrap();

        assert!(cache.is_empty());
    }

    #[test]
    fn ignores_unknown_cache_versions() {
        let dir = TempDir::new();
        fs::write(
            dir.cache_path(),
            br#"{"version": 99, "entries": {"author:1": {"name": "old"}}}"#,
        )
        .unwrap();

        let cache = Cache::open_at(dir.cache_path()).unwrap();

        assert!(cache.is_empty());
    }
}
