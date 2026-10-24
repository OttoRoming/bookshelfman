//! Persistent caching for StoryGraph searches.
//!
//! Search results are stored in the application-wide [`Cache`] under
//! `storygraph:search:<normalized query>` keys. Normalization ignores case and
//! repeated/leading/trailing whitespace, so equivalent queries share one
//! entry.

use std::future::Future;

use super::{BookPane, Result};
use crate::cache::{self, Cache};

const KEY_PREFIX: &str = "storygraph:search:";

/// Normalizes a query for use in a cache key.
fn normalize_query(query: &str) -> String {
    query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Returns the cache key for `query`.
pub(crate) fn key(query: &str) -> String {
    format!("{KEY_PREFIX}{}", normalize_query(query))
}

/// Looks up search results for `query`.
pub(crate) fn get(cache: &Cache, query: &str) -> Option<Vec<BookPane>> {
    cache.get(&key(query))
}

/// Stores search results, replacing any previous entry for the same query.
pub(crate) fn put(cache: &Cache, query: &str, books: &[BookPane]) -> cache::Result<()> {
    cache.set(key(query), &books.to_vec())
}

/// Returns cached results for `query`, or calls `fetch` and caches its result.
///
/// Empty result sets are cached too. Cache write failures are returned as
/// errors; the freshly fetched results are not returned in that case.
pub(crate) async fn get_or_fetch<F, Fut>(
    cache: &Cache,
    query: &str,
    fetch: F,
) -> Result<Vec<BookPane>>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Vec<BookPane>>>,
{
    if let Some(books) = get(cache, query) {
        return Ok(books);
    }

    let books = fetch().await?;
    put(cache, query, &books)?;

    Ok(books)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use url::Url;
    use uuid::Uuid;

    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "bookshelfman-storygraph-cache-test-{}",
                Uuid::new_v4()
            ));
            fs::create_dir_all(&path).unwrap();

            Self(path)
        }

        fn cache_path(&self) -> PathBuf {
            self.0.join("cache.json")
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn book(title: &str) -> BookPane {
        BookPane {
            id: Uuid::new_v4(),
            authors: vec![Uuid::new_v4()],
            title: title.to_string(),
            cover_url: Some(Url::parse("https://example.com/cover.jpg").unwrap()),
        }
    }

    #[test]
    fn normalizes_queries_in_keys() {
        assert_eq!(
            key("  The   Great\tGatsby "),
            "storygraph:search:the great gatsby"
        );
        assert_eq!(key("ÜRSULA"), "storygraph:search:ürsula");
    }

    #[test]
    fn round_trips_results_through_the_cache_file() {
        let dir = TempDir::new();
        let path = dir.cache_path();
        let expected = vec![book("The Dispossessed")];

        {
            let cache = Cache::open_at(path.clone()).unwrap();
            put(&cache, "The Dispossessed", &expected).unwrap();
            cache.flush().unwrap();
        }

        let cache = Cache::open_at(path).unwrap();

        assert_eq!(get(&cache, "the   dispossessed"), Some(expected));
    }

    #[test]
    fn empty_results_are_cached() {
        let dir = TempDir::new();
        let cache = Cache::open_at(dir.cache_path()).unwrap();

        put(&cache, "nothing", &[]).unwrap();

        assert_eq!(get(&cache, "NOTHING"), Some(Vec::new()));
    }

    #[tokio::test]
    async fn a_cache_hit_skips_the_fetch() {
        let dir = TempDir::new();
        let cache = Cache::open_at(dir.cache_path()).unwrap();
        let calls = AtomicUsize::new(0);
        let expected = vec![book("Kindred")];

        let first = get_or_fetch(&cache, "Kindred", || async {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(expected.clone())
        })
        .await
        .unwrap();

        let second = get_or_fetch(&cache, "  kindred  ", || async {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(vec![book("unexpected")])
        })
        .await
        .unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(first, expected);
        assert_eq!(second, expected);
    }
}
