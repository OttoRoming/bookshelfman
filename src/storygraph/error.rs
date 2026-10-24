use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("HTTP request failed: {0}")]
    Http(#[from] wreq::Error),

    #[error("unexpected status code: {status}")]
    Status { status: u16 },

    #[error("unexpected content type: {0}")]
    ContentType(String),

    #[error("required cookie not found")]
    MissingCookie,

    #[error("failed to parse URL: {0}")]
    Url(#[from] url::ParseError),

    #[error("failed to parse UUID at index {index}: {source}")]
    Uuid {
        index: usize,
        #[source]
        source: uuid::Error,
    },

    #[error("author IDs are not valid UTF-8")]
    AuthorIds,

    #[error(transparent)]
    Cache(#[from] crate::cache::Error),
}
