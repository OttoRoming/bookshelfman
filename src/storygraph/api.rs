use std::fmt;

use url::Url;
use uuid::Uuid;

/// An author on StoryGraph.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Author {
    pub id: Uuid,
    pub name: String,
}

impl fmt::Display for Author {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

/// A book together with its resolved authors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Book {
    pub id: Uuid,
    pub title: String,
    pub authors: Vec<Author>,
    pub cover_url: String,
}

impl fmt::Display for Book {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let authors = self
            .authors
            .iter()
            .map(Author::to_string)
            .collect::<Vec<_>>()
            .join(", ");

        write!(f, "{} - {}", self.id, authors)
    }
}

/// A single search result as scraped from StoryGraph's browse page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookPane {
    pub id: Uuid,
    pub authors: Vec<Uuid>,
    pub title: String,
    pub cover_url: Option<Url>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOOK_ID: &str = "0e1a6b9c-3f2d-4a5b-8c7d-9e0f1a2b3c4d";
    const AUTHOR_ONE: &str = "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d";
    const AUTHOR_TWO: &str = "2b3c4d5e-6f7a-4b8c-9d0e-1f2a3b4c5d6e";

    fn author(id: &str) -> Author {
        Author {
            id: Uuid::parse_str(id).unwrap(),
            name: "Ursula K. Le Guin".to_owned(),
        }
    }

    #[test]
    fn author_displays_its_id() {
        assert_eq!(author(AUTHOR_ONE).to_string(), AUTHOR_ONE);
    }

    #[test]
    fn book_displays_its_id_and_author_ids() {
        let book = Book {
            id: Uuid::parse_str(BOOK_ID).unwrap(),
            title: "The Dispossessed".to_owned(),
            authors: vec![author(AUTHOR_ONE), author(AUTHOR_TWO)],
            cover_url: "https://cdn.thestorygraph.com/covers/dispossessed.jpg".to_owned(),
        };

        assert_eq!(
            book.to_string(),
            format!("{BOOK_ID} - {AUTHOR_ONE}, {AUTHOR_TWO}")
        );
    }
}
