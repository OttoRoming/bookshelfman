//! Items shown by a [`SearchSelect`](super::SearchSelect).

use std::borrow::Cow;

/// Extra information shown next to an item in the preview pane.
///
/// Metadata labels and values are also included in search matching, so an item
/// can be found by, for example, its author or publication year.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Metadata {
    /// The name of the field, such as `"Author"`.
    pub label: String,
    /// The value of the field, such as `"Ursula K. Le Guin"`.
    pub value: String,
}

impl Metadata {
    /// Creates a new metadata field.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

impl From<(&str, &str)> for Metadata {
    fn from((label, value): (&str, &str)) -> Self {
        Self::new(label, value)
    }
}

impl From<(String, String)> for Metadata {
    fn from((label, value): (String, String)) -> Self {
        Self::new(label, value)
    }
}

/// A value that can be picked from a [`SearchSelect`](super::SearchSelect).
///
/// The label is the primary text shown in the list and is what the user types
/// to search. Metadata is displayed in the preview pane and is searched too.
///
/// ```
/// use bookshelfman::forms::search_select::{Metadata, SelectItem};
///
/// struct Book {
///     title: String,
///     metadata: Vec<Metadata>,
/// }
///
/// impl SelectItem for Book {
///     fn label(&self) -> &str {
///         &self.title
///     }
///
///     fn metadata(&self) -> &[Metadata] {
///         &self.metadata
///     }
/// }
/// ```
pub trait SelectItem {
    /// The text displayed in the result list and matched against the query.
    fn label(&self) -> &str;

    /// Additional details displayed in the preview pane.
    ///
    /// The default implementation returns no metadata.
    fn metadata(&self) -> &[Metadata] {
        &[]
    }
}

impl SelectItem for str {
    fn label(&self) -> &str {
        self
    }
}

impl SelectItem for String {
    fn label(&self) -> &str {
        self
    }
}

impl SelectItem for Cow<'_, str> {
    fn label(&self) -> &str {
        self
    }
}

impl<T: SelectItem + ?Sized> SelectItem for &T {
    fn label(&self) -> &str {
        (*self).label()
    }

    fn metadata(&self) -> &[Metadata] {
        (*self).metadata()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_are_selectable() {
        assert_eq!("hello".label(), "hello");
        assert_eq!(String::from("hello").label(), "hello");
        assert_eq!(Cow::Borrowed("hello").label(), "hello");
        assert!(String::from("hello").metadata().is_empty());
    }

    #[test]
    fn metadata_converts_from_tuples() {
        let metadata = Metadata::from(("Author", "Ursula K. Le Guin"));

        assert_eq!(metadata.label, "Author");
        assert_eq!(metadata.value, "Ursula K. Le Guin");
    }
}
