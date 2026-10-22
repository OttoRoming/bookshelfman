mod api;
mod cache;
mod client;
mod error;
mod search;

pub use api::{Author, Book, BookPane};
pub use cache::Cache;
pub use client::Storygraph;
pub use error::{Error, Result};

impl Storygraph {
    /// Searches StoryGraph for books matching `query`.
    ///
    /// The returned panes contain the book ID, title, cover URL, and the IDs of
    /// its authors.
    pub async fn search(&self, query: &str) -> Result<Vec<BookPane>> {
        let body = self.fetch("/browse", &[("search_term", query)]).await?;

        Ok(search::scrape_search(&body))
    }
}
