use std::path::PathBuf;

use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to read EPUB {path:?}")]
    Epub {
        path: PathBuf,
        #[source]
        source: epub::doc::DocError,
    },

    #[error("could not determine a title for {path:?}")]
    MissingTitle { path: PathBuf },

    #[error("unsupported book format for {path:?}")]
    UnsupportedFormat { path: PathBuf },
}
