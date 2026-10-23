//! Format-neutral book metadata.
//!
//! [`BookData`] is the representation the rest of the application uses for
//! information read from a book file. Each format has its own small reader
//! module; adding support for another format means adding a [`Format`] variant,
//! the matching arms in [`Format::from_path`] and [`Format::read`], and a
//! reader module such as the existing `epub` one.
//!
//! ```no_run
//! use bookshelfman::bookdata::BookData;
//!
//! let book = BookData::open("/home/user/Books/dispossessed.epub")?;
//! println!("{} ({} pages)", book.title, book.page_count);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod epub;
mod error;

use std::path::Path;

pub use error::{Error, Result};

/// A book format understood by [`BookData::open`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// The EPUB format.
    Epub,
}

impl Format {
    /// Detects the format of `path` from its file extension.
    ///
    /// The extension is matched case-insensitively.
    pub fn from_path(path: impl AsRef<Path>) -> Option<Self> {
        let extension = path.as_ref().extension()?.to_str()?;

        match extension.to_ascii_lowercase().as_str() {
            "epub" => Some(Self::Epub),
            // "pdf" => Some(Self::Pdf),
            _ => None,
        }
    }

    /// Reads book metadata from `path` using this format.
    pub fn read(self, path: impl AsRef<Path>) -> Result<BookData> {
        let path = path.as_ref();

        match self {
            Self::Epub => epub::read(path),
            // Self::Pdf => pdf::read(path),
        }
    }
}

/// Metadata extracted from a book file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookData {
    /// The title of the book.
    pub title: String,

    /// The number of pages in the book.
    pub page_count: Option<usize>,

    /// The cover image, when the file contains one.
    pub cover: Option<CoverImage>,
}

impl BookData {
    /// Opens the book at `path` and extracts its metadata.
    ///
    /// [`Format::from_path`] chooses the reader from the file extension.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let format = Format::from_path(path).ok_or_else(|| Error::UnsupportedFormat {
            path: path.to_owned(),
        })?;

        format.read(path)
    }
}

/// A cover image in its original encoded form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverImage {
    /// The encoded image bytes, such as the contents of a JPEG or PNG file.
    pub bytes: Vec<u8>,

    /// The MIME type of the image, such as `"image/jpeg"`.
    pub mime_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "bookshelfman-bookdata-test-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&path).unwrap();

            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn detects_formats_case_insensitively() {
        assert_eq!(
            Format::from_path("books/dispossessed.EPUB"),
            Some(Format::Epub)
        );
    }

    #[test]
    fn does_not_detect_unsupported_formats() {
        assert_eq!(Format::from_path("books/dispossessed.mobi"), None);
    }

    #[test]
    fn rejects_unsupported_formats() {
        let dir = TempDir::new();
        let path = dir.0.join("dispossessed.mobi");
        fs::write(&path, b"not a supported book").unwrap();

        let error = BookData::open(&path).unwrap_err();

        assert!(matches!(error, Error::UnsupportedFormat { .. }));
    }

    #[test]
    fn rejects_paths_without_an_extension() {
        let dir = TempDir::new();
        let path = dir.0.join("dispossessed");
        fs::write(&path, b"not a supported book").unwrap();

        let error = BookData::open(&path).unwrap_err();

        assert!(matches!(error, Error::UnsupportedFormat { .. }));
    }
}
