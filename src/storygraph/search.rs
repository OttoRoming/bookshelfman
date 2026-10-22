use std::sync::LazyLock;

use scraper::{Html, Selector};
use url::Url;
use uuid::Uuid;

use super::BookPane;
use super::client::split_uuids;

static BOOK_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div[data-book-id][data-author-ids]").unwrap());
static TITLE_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".book-title-author-and-series h3 a").unwrap());
static COVER_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".book-cover a img").unwrap());

pub(crate) fn scrape_search(body: &str) -> Vec<BookPane> {
    let document = Html::parse_document(body);
    let mut books = Vec::new();

    for element in document.select(&BOOK_SELECTOR) {
        let Some(id) = element
            .value()
            .attr("data-book-id")
            .and_then(|value| Uuid::parse_str(value).ok())
        else {
            continue;
        };

        let title = element
            .select(&TITLE_SELECTOR)
            .last()
            .map(|title| title.text().collect())
            .unwrap_or_default();

        let cover_url = element
            .select(&COVER_SELECTOR)
            .last()
            .and_then(|cover| cover.value().attr("src"))
            .and_then(|src| Url::parse(src).ok());

        let Ok(authors) = split_uuids(element.value().attr("data-author-ids").unwrap_or("")) else {
            continue;
        };

        books.push(BookPane {
            id,
            authors,
            title,
            cover_url,
        });
    }

    books
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOOK_ID: &str = "0e1a6b9c-3f2d-4a5b-8c7d-9e0f1a2b3c4d";
    const AUTHOR_ONE: &str = "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d";
    const AUTHOR_TWO: &str = "2b3c4d5e-6f7a-4b8c-9d0e-1f2a3b4c5d6e";
    const INVALID_AUTHOR_IDS: &str = "zzzzzzzz-zzzz-zzzz-zzzz-zzzzzzzzzzzz";

    fn search_response() -> String {
        format!(
            r#"
            <html><body>
              <div data-book-id="{BOOK_ID}" data-author-ids="{AUTHOR_ONE},{AUTHOR_TWO}">
                <div class="book-title-author-and-series">
                  <h3><a href="/books/{BOOK_ID}">The Dispossessed</a></h3>
                </div>
                <div class="book-cover">
                  <a href="/books/{BOOK_ID}">
                    <img src="https://cdn.thestorygraph.com/covers/dispossessed.jpg">
                  </a>
                </div>
              </div>
              <div data-book-id="not-a-uuid" data-author-ids="{AUTHOR_ONE}"></div>
              <div data-book-id="3c4d5e6f-7a8b-4c9d-0e1f-2a3b4c5d6e7f" data-author-ids="{INVALID_AUTHOR_IDS}"></div>
            </body></html>
            "#
        )
    }

    #[test]
    fn scrapes_books_from_a_search_page() {
        let books = scrape_search(&search_response());

        assert_eq!(books.len(), 1);

        let book = &books[0];
        assert_eq!(book.id, Uuid::parse_str(BOOK_ID).unwrap());
        assert_eq!(book.title, "The Dispossessed");
        assert_eq!(
            book.authors,
            vec![
                Uuid::parse_str(AUTHOR_ONE).unwrap(),
                Uuid::parse_str(AUTHOR_TWO).unwrap()
            ]
        );
        assert_eq!(
            book.cover_url.as_ref().map(Url::as_str),
            Some("https://cdn.thestorygraph.com/covers/dispossessed.jpg")
        );
    }
}
