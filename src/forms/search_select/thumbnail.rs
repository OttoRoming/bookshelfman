//! Thumbnail previews for search select items.

use ratatui::text::Line;

/// The preview shown next to the selected item.
///
/// A thumbnail is a small block of styled terminal text. It can contain any
/// ratatui [`Line`]s, so callers can draw ASCII art, Unicode half-blocks with
/// colors, or any other preview they can build. The lines are clipped to the
/// preview pane, so they do not need to be sized exactly.
///
/// ```
/// use bookshelfman::forms::search_select::Thumbnail;
///
/// let thumbnail = Thumbnail::from_text(" .-------.\n | COVER |\n '-------.'");
///
/// assert_eq!(thumbnail.height(), 3);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Thumbnail {
    lines: Vec<Line<'static>>,
}

impl Thumbnail {
    /// Creates an empty thumbnail.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a thumbnail from a block of plain text.
    ///
    /// The text is split on line boundaries. This is the simplest way to
    /// provide an ASCII-art preview.
    pub fn from_text(text: impl AsRef<str>) -> Self {
        Self {
            lines: text
                .as_ref()
                .lines()
                .map(|line| Line::raw(line.to_string()))
                .collect(),
        }
    }

    /// Creates a thumbnail from styled lines.
    ///
    /// ```
    /// use bookshelfman::forms::search_select::Thumbnail;
    /// use ratatui::style::{Color, Style};
    /// use ratatui::text::{Line, Span};
    ///
    /// let thumbnail = Thumbnail::from_lines([
    ///     Line::from(Span::styled("COVER", Style::new().fg(Color::Cyan))),
    /// ]);
    ///
    /// assert_eq!(thumbnail.width(), 5);
    /// ```
    pub fn from_lines(lines: impl IntoIterator<Item = impl Into<Line<'static>>>) -> Self {
        Self {
            lines: lines.into_iter().map(Into::into).collect(),
        }
    }

    /// Appends one line.
    pub fn push_line(&mut self, line: impl Into<Line<'static>>) {
        self.lines.push(line.into());
    }

    /// Returns the thumbnail's lines.
    pub fn lines(&self) -> &[Line<'static>] {
        &self.lines
    }

    /// Returns whether the thumbnail contains no lines.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Returns the number of lines in the thumbnail.
    pub fn height(&self) -> u16 {
        u16::try_from(self.lines.len()).unwrap_or(u16::MAX)
    }

    /// Returns the display width of the widest line.
    pub fn width(&self) -> u16 {
        let width = self.lines.iter().map(Line::width).max().unwrap_or(0);

        u16::try_from(width).unwrap_or(u16::MAX)
    }
}

impl From<&str> for Thumbnail {
    fn from(text: &str) -> Self {
        Self::from_text(text)
    }
}

impl From<String> for Thumbnail {
    fn from(text: String) -> Self {
        Self::from_text(text)
    }
}

impl From<Vec<Line<'static>>> for Thumbnail {
    fn from(lines: Vec<Line<'static>>) -> Self {
        Self { lines }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::text::Span;

    #[test]
    fn splits_text_into_lines() {
        let thumbnail = Thumbnail::from_text("one\r\ntwo\nthree");

        assert_eq!(thumbnail.height(), 3);
        assert_eq!(thumbnail.width(), 5);
        assert_eq!(thumbnail.lines()[0].spans[0].content, "one");
    }

    #[test]
    fn empty_text_produces_an_empty_thumbnail() {
        let thumbnail = Thumbnail::from_text("");

        assert!(thumbnail.is_empty());
        assert_eq!(thumbnail.height(), 0);
        assert_eq!(thumbnail.width(), 0);
    }

    #[test]
    fn styled_lines_are_preserved() {
        let thumbnail = Thumbnail::from_lines([Line::from(Span::raw("cover"))]);

        assert_eq!(thumbnail.lines()[0].spans[0].content, "cover");
    }
}
