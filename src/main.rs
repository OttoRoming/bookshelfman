use std::sync::Arc;

use bookshelfman::forms::{Metadata, SearchError, SearchSelect, SelectItem, Thumbnail};
use bookshelfman::storygraph::{BookPane, Storygraph};

/// A StoryGraph search result with the extra details shown by the form.
struct SearchResult {
    pane: BookPane,
    metadata: Vec<Metadata>,
}

impl SearchResult {
    fn new(pane: BookPane) -> Self {
        let mut metadata = vec![Metadata::new("Book ID", pane.id.to_string())];

        if let Some(url) = &pane.cover_url {
            metadata.push(Metadata::new("Cover", url.as_str()));
        }

        Self { pane, metadata }
    }
}

impl SelectItem for SearchResult {
    fn label(&self) -> &str {
        &self.pane.title
    }

    fn metadata(&self) -> &[Metadata] {
        &self.metadata
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let storygraph = Arc::new(Storygraph::new().await?);
    // The form runs searches on its own worker thread. Use the current Tokio
    // handle from that thread so the async StoryGraph client can be awaited.
    let runtime = tokio::runtime::Handle::current();

    let selected = SearchSelect::search("Pick a book", move |query| {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let storygraph = Arc::clone(&storygraph);

        runtime
            .block_on(async move { storygraph.search(&query).await })
            .map(|panes| panes.into_iter().map(SearchResult::new).collect::<Vec<_>>())
            .map_err(|error| -> SearchError { Box::new(std::io::Error::other(error.to_string())) })
    })
    .with_initial_query("the great gatsby")
    // Fetching and decoding cover art is not wired up yet, so show a
    // placeholder for results that have a cover.
    .with_thumbnail(|book| {
        book.pane
            .cover_url
            .as_ref()
            .map(|_| Thumbnail::from_text("┌─────────┐\n│  cover  │\n└─────────┘"))
    })
    .run()?;

    if let Some(book) = selected {
        println!("selected: {} ({})", book.pane.title, book.pane.id);
    }

    Ok(())
}
