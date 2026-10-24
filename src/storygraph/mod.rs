mod api;
mod client;
mod error;
mod search;
mod search_cache;

pub use api::{Author, Book, BookPane};
pub use client::Storygraph;
pub use error::{Error, Result};

impl Storygraph {
    /// Searches StoryGraph for books matching `query`.
    ///
    /// The returned panes contain the book ID, title, cover URL, and the IDs of
    /// its authors.
    ///
    /// Results are cached persistently under a case- and whitespace-insensitive
    /// query key. There is no automatic expiry; use [`Storygraph::cache`] to
    /// inspect or clear the cache.
    pub async fn search(&self, query: &str) -> Result<Vec<BookPane>> {
        search_cache::get_or_fetch(self.cache(), query, || async {
            let body = self.fetch("/browse", &[("search_term", query)]).await?;

            Ok(search::scrape_search(&body))
        })
        .await
    }
}
