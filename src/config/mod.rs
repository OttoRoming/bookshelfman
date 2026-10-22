//! User configuration.
//!
//! The configuration file lives at
//! `dirs::config_dir()/bookshelfman/bookshelfman.toml` — on Linux this is
//! usually `~/.config/bookshelfman/bookshelfman.toml`.
//!
//! ```toml
//! bookshelf_dir = "/home/user/Books"
//! ```

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const CONFIG_DIR: &str = "bookshelfman";
const CONFIG_FILE: &str = "bookshelfman.toml";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not determine the user config directory")]
    NoConfigDir,

    #[error("failed to read config file {path:?}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to parse config file {path:?}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// The application configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// Directory containing the user's bookshelf.
    pub bookshelf_dir: PathBuf,
}

impl Config {
    /// Returns the default configuration path for the current platform.
    pub fn default_path() -> Result<PathBuf> {
        let dir = dirs::config_dir().ok_or(Error::NoConfigDir)?;

        Ok(dir.join(CONFIG_DIR).join(CONFIG_FILE))
    }

    /// Loads the configuration from the platform's config directory.
    pub fn load() -> Result<Self> {
        Self::load_from(Self::default_path()?)
    }

    /// Loads the configuration from a specific path.
    pub fn load_from(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        let contents = fs::read_to_string(&path).map_err(|source| Error::Read {
            path: path.clone(),
            source,
        })?;

        let mut config: Self = toml::from_str(&contents).map_err(|source| Error::Parse {
            path: path.clone(),
            source,
        })?;

        config.bookshelf_dir = expand_tilde(config.bookshelf_dir);

        Ok(config)
    }
}

/// Expands a leading `~` to the user's home directory.
fn expand_tilde(path: PathBuf) -> PathBuf {
    if let Ok(rest) = path.strip_prefix("~")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }

    path
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("bookshelfman-config-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();

            Self(path)
        }

        fn config_path(&self) -> PathBuf {
            self.0.join(CONFIG_FILE)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn loads_bookshelf_dir() {
        let dir = TempDir::new();
        let path = dir.config_path();
        fs::write(&path, "bookshelf_dir = \"/tmp/books\"\n").unwrap();

        let config = Config::load_from(&path).unwrap();

        assert_eq!(config.bookshelf_dir, PathBuf::from("/tmp/books"));
    }

    #[test]
    fn expands_a_leading_tilde() {
        let Some(home) = dirs::home_dir() else {
            return;
        };

        let dir = TempDir::new();
        let path = dir.config_path();
        fs::write(&path, "bookshelf_dir = \"~/Books\"\n").unwrap();

        let config = Config::load_from(&path).unwrap();

        assert_eq!(config.bookshelf_dir, home.join("Books"));
    }

    #[test]
    fn reports_a_missing_bookshelf_dir() {
        let dir = TempDir::new();
        let path = dir.config_path();
        fs::write(&path, "something_else = true\n").unwrap();

        let error = Config::load_from(&path).unwrap_err();

        assert!(matches!(error, Error::Parse { .. }));
    }

    #[test]
    fn reports_a_missing_file() {
        let dir = TempDir::new();
        let error = Config::load_from(dir.config_path()).unwrap_err();

        assert!(matches!(error, Error::Read { .. }));
    }
}
