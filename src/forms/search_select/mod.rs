//! A searchable single-select form rendered inline.
//!
//! [`SearchSelect`] takes a prompt and a list of [`SelectItem`]s, lets the user
//! filter the list by typing, and returns the selected item. It is similar in
//! spirit to `inquire`'s select prompt or Go's `huh` select field, with one
//! important difference: the form renders at the current cursor row and
//! **never enters the alternate screen**. Existing terminal output stays
//! visible above the form.
//!
//! Items may expose [`Metadata`] (shown in a preview pane and included in
//! search matching), and a thumbnail callback can provide an arbitrary
//! terminal-text preview for the highlighted item:
//!
//! ```no_run
//! use bookshelfman::forms::search_select::{
//!     Metadata, SearchSelect, SelectItem, Thumbnail,
//! };
//!
//! struct Book {
//!     title: String,
//!     metadata: Vec<Metadata>,
//!     cover: String,
//! }
//!
//! impl SelectItem for Book {
//!     fn label(&self) -> &str {
//!         &self.title
//!     }
//!
//!     fn metadata(&self) -> &[Metadata] {
//!         &self.metadata
//!     }
//! }
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let books = vec![Book {
//!     title: "The Dispossessed".to_string(),
//!     metadata: vec![Metadata::new("Author", "Ursula K. Le Guin")],
//!     cover: " .-------.\n | COVER |\n '-------.'".to_string(),
//! }];
//!
//! let selected = SearchSelect::new("Pick a book", books)
//!     .with_thumbnail(|book| Some(Thumbnail::from_text(&book.cover)))
//!     .run()?;
//!
//! if let Some(book) = selected {
//!     println!("selected: {}", book.title);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Controls
//!
//! | Key | Action |
//! | --- | --- |
//! | Type | Filter the list |
//! | <kbd>↑</kbd> / <kbd>↓</kbd> | Move the selection |
//! | <kbd>PageUp</kbd> / <kbd>PageDown</kbd> | Move a page at a time |
//! | <kbd>←</kbd> / <kbd>→</kbd> | Move the input cursor |
//! | <kbd>Home</kbd> / <kbd>End</kbd> | Move to the start / end of the query |
//! | <kbd>Backspace</kbd> / <kbd>Delete</kbd> | Delete text |
//! | <kbd>Ctrl+U</kbd> | Clear the query |
//! | <kbd>Ctrl+W</kbd> | Delete the previous word |
//! | <kbd>Enter</kbd> | Select the highlighted item |
//! | <kbd>Esc</kbd> / <kbd>Ctrl+C</kbd> | Cancel |
//!
//! [`run`](SearchSelect::run) returns `Ok(None)` when the user cancels, and an
//! [`Error`] if the terminal cannot be used.

mod item;
mod render;
mod state;
mod terminal;
mod thumbnail;

pub use item::{Metadata, SelectItem};
pub use thumbnail::Thumbnail;

use std::collections::HashMap;
use std::io;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Position;
use ratatui::widgets::ListState;

use state::{EntryInfo, Input, moved_selection, rank_matches};
use terminal::Session;

/// The default number of result rows.
const DEFAULT_PAGE_SIZE: usize = 8;
/// The largest number of result rows that can be requested.
const MAX_PAGE_SIZE: usize = 50;

/// Errors returned by a search select form.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The form needs an interactive terminal on both stdin and stdout.
    #[error("search select requires an interactive terminal (stdin and stdout must be TTYs)")]
    NotATerminal,

    /// The terminal does not have enough room for the form.
    #[error(
        "terminal is too small for search select: need at least {min_width}x{min_height}, got {width}x{height}"
    )]
    TerminalTooSmall {
        /// The terminal width in columns.
        width: u16,
        /// The terminal height in rows.
        height: u16,
        /// The minimum supported width.
        min_width: u16,
        /// The minimum supported height.
        min_height: u16,
    },

    /// A terminal operation failed.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// A convenience result type for search select operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// How the event loop finished.
enum Outcome {
    Selected(usize),
    Cancelled,
}

/// Builds a thumbnail for an item. See [`SearchSelect::with_thumbnail`].
type ThumbnailCallback<'a, T> = Box<dyn Fn(&T) -> Option<Thumbnail> + 'a>;

/// An inline searchable select form.
///
/// Build the form with [`SearchSelect::new`], optionally attach a thumbnail
/// callback and other options, then call [`run`](SearchSelect::run).
pub struct SearchSelect<'a, T> {
    prompt: String,
    items: Vec<T>,
    entries: Vec<EntryInfo>,
    thumbnail: Option<ThumbnailCallback<'a, T>>,
    thumbnails: HashMap<usize, Option<Thumbnail>>,
    input: Input,
    matches: Vec<usize>,
    selected: usize,
    list_state: ListState,
    page_size: usize,
    empty_message: String,
    help: Option<String>,
    has_metadata: bool,
    cursor_pos: Option<Position>,
    visible_rows: usize,
}

impl<'a, T: SelectItem> SearchSelect<'a, T> {
    /// Creates a search select form.
    ///
    /// `prompt` is the question shown above the search field. `items` are the
    /// selectable values; their labels and metadata are read once here.
    pub fn new(prompt: impl Into<String>, items: impl IntoIterator<Item = T>) -> Self {
        let items = items.into_iter().collect::<Vec<_>>();
        let entries = items
            .iter()
            .map(|item| EntryInfo::new(item))
            .collect::<Vec<_>>();
        let matches = (0..items.len()).collect::<Vec<_>>();
        let has_metadata = entries.iter().any(|entry| !entry.metadata.is_empty());
        let mut list_state = ListState::default();
        list_state.select((!matches.is_empty()).then_some(0));

        Self {
            prompt: prompt.into(),
            items,
            entries,
            thumbnail: None,
            thumbnails: HashMap::new(),
            input: Input::default(),
            matches,
            selected: 0,
            list_state,
            page_size: DEFAULT_PAGE_SIZE,
            empty_message: "No matches".to_string(),
            help: None,
            has_metadata,
            cursor_pos: None,
            visible_rows: 0,
        }
    }

    /// Sets the number of result rows shown at once (default: 8).
    ///
    /// The value is clamped to the range `1..=50`. If the terminal is shorter
    /// than requested, the form shows as many rows as fit.
    pub fn with_page_size(mut self, page_size: usize) -> Self {
        self.page_size = page_size.clamp(1, MAX_PAGE_SIZE);
        self
    }

    /// Sets the callback that builds a thumbnail for an item.
    ///
    /// The callback is invoked lazily the first time an item is highlighted
    /// and the result is cached, so it is safe to do work such as decoding an
    /// image or loading a cached cover. Return `None` to show the "no preview"
    /// placeholder.
    pub fn with_thumbnail(mut self, callback: impl Fn(&T) -> Option<Thumbnail> + 'a) -> Self {
        self.thumbnail = Some(Box::new(callback));
        self.thumbnails.clear();
        self
    }

    /// Sets the message shown when a non-empty query matches no items.
    pub fn with_empty_message(mut self, message: impl Into<String>) -> Self {
        self.empty_message = message.into();
        self
    }

    /// Replaces the help line shown below the form.
    pub fn with_help_message(mut self, message: impl Into<String>) -> Self {
        self.help = Some(message.into());
        self
    }

    /// Runs the form and returns the selected item.
    ///
    /// Returns `Ok(None)` if the user cancels with <kbd>Esc</kbd> or
    /// <kbd>Ctrl+C</kbd>. The form renders inline and is removed from the
    /// terminal before this method returns.
    pub fn run(mut self) -> Result<Option<T>> {
        let form_height = u16::try_from(self.page_size.saturating_add(5)).unwrap_or(u16::MAX);
        let mut session = Session::start(form_height)?;
        let outcome = self.event_loop(&mut session);

        drop(session);

        match outcome? {
            Some(index) if index < self.items.len() => Ok(Some(self.items.swap_remove(index))),
            Some(_) | None => Ok(None),
        }
    }

    fn event_loop(&mut self, session: &mut Session) -> Result<Option<usize>> {
        loop {
            session.draw(|frame| self.render(frame))?;

            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Release {
                        continue;
                    }

                    match self.handle_key(key) {
                        Some(Outcome::Selected(index)) => return Ok(Some(index)),
                        Some(Outcome::Cancelled) => return Ok(None),
                        None => {}
                    }
                }
                Event::Paste(text) => {
                    self.input.insert_str(&text);
                    self.refilter();
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<Outcome> {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            KeyCode::Esc => return Some(Outcome::Cancelled),
            KeyCode::Char('c') if control => return Some(Outcome::Cancelled),
            KeyCode::Enter => {
                return self
                    .matches
                    .get(self.selected)
                    .copied()
                    .map(Outcome::Selected);
            }
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::PageUp => self.move_selection(-(self.visible_rows.max(1) as isize)),
            KeyCode::PageDown => self.move_selection(self.visible_rows.max(1) as isize),
            KeyCode::Home if control => self.select_index(0),
            KeyCode::End if control => self.select_index(self.matches.len().saturating_sub(1)),
            KeyCode::Home => self.input.home(),
            KeyCode::End => self.input.end(),
            KeyCode::Left => self.input.move_left(),
            KeyCode::Right => self.input.move_right(),
            KeyCode::Backspace if control => {
                if self.input.delete_word() {
                    self.refilter();
                }
            }
            KeyCode::Backspace => {
                if self.input.backspace() {
                    self.refilter();
                }
            }
            KeyCode::Delete => {
                if self.input.delete() {
                    self.refilter();
                }
            }
            KeyCode::Char('u') if control => {
                self.input.clear();
                self.refilter();
            }
            KeyCode::Char('w') if control => {
                if self.input.delete_word() {
                    self.refilter();
                }
            }
            KeyCode::Char('a') if control => self.input.home(),
            KeyCode::Char('e') if control => self.input.end(),
            KeyCode::Char(ch) if !control && !key.modifiers.contains(KeyModifiers::ALT) => {
                self.input.insert_char(ch);
                self.refilter();
            }
            _ => {}
        }

        None
    }

    fn refilter(&mut self) {
        self.matches = rank_matches(&self.entries, &self.input.value);
        self.selected = 0;
        *self.list_state.offset_mut() = 0;
        self.list_state
            .select((!self.matches.is_empty()).then_some(0));
    }

    fn move_selection(&mut self, delta: isize) {
        if self.matches.is_empty() {
            return;
        }

        self.selected = moved_selection(self.selected, self.matches.len(), delta);
        self.list_state.select(Some(self.selected));
    }

    fn select_index(&mut self, index: usize) {
        if self.matches.is_empty() {
            return;
        }

        self.selected = index.min(self.matches.len() - 1);
        self.list_state.select(Some(self.selected));
    }

    fn ensure_selected_thumbnail(&mut self) {
        let Some(&index) = self.matches.get(self.selected) else {
            return;
        };

        if self.thumbnails.contains_key(&index) {
            return;
        }

        let Some(callback) = self.thumbnail.as_ref() else {
            return;
        };

        let thumbnail = callback(&self.items[index]);
        self.thumbnails.insert(index, thumbnail);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Book {
        title: String,
        metadata: Vec<Metadata>,
    }

    impl Book {
        fn new(title: &str, author: &str) -> Self {
            Self {
                title: title.to_string(),
                metadata: vec![Metadata::new("Author", author)],
            }
        }
    }

    impl SelectItem for Book {
        fn label(&self) -> &str {
            &self.title
        }

        fn metadata(&self) -> &[Metadata] {
            &self.metadata
        }
    }

    fn books() -> Vec<Book> {
        vec![
            Book::new("The Dispossessed", "Ursula K. Le Guin"),
            Book::new("The Great Gatsby", "F. Scott Fitzgerald"),
            Book::new("Kindred", "Octavia E. Butler"),
        ]
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn enter_selects_the_highlighted_item() {
        let mut form = SearchSelect::new("Pick a book", books());
        form.move_selection(1);

        let outcome = form.handle_key(key(KeyCode::Enter));

        assert!(matches!(outcome, Some(Outcome::Selected(1))));
    }

    #[test]
    fn escape_cancels() {
        let mut form = SearchSelect::new("Pick a book", books());

        let outcome = form.handle_key(key(KeyCode::Esc));

        assert!(matches!(outcome, Some(Outcome::Cancelled)));
    }

    #[test]
    fn enter_does_nothing_when_nothing_matches() {
        let mut form = SearchSelect::new("Pick a book", books());
        form.input.insert_str("zzz");
        form.refilter();

        assert!(form.handle_key(key(KeyCode::Enter)).is_none());
    }

    #[test]
    fn typing_filters_and_resets_the_selection() {
        let mut form = SearchSelect::new("Pick a book", books());
        form.move_selection(2);

        form.handle_key(key(KeyCode::Char('k')));
        form.handle_key(key(KeyCode::Char('i')));
        form.handle_key(key(KeyCode::Char('n')));
        form.handle_key(key(KeyCode::Char('d')));

        assert_eq!(form.input.value, "kind");
        assert_eq!(form.matches.len(), 1);
        assert_eq!(form.selected, 0);
        assert!(matches!(
            form.handle_key(key(KeyCode::Enter)),
            Some(Outcome::Selected(2))
        ));
    }

    #[test]
    fn ctrl_u_clears_the_query() {
        let mut form = SearchSelect::new("Pick a book", books());
        form.input.insert_str("zzz");
        form.refilter();

        form.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));

        assert!(form.input.value.is_empty());
        assert_eq!(form.matches.len(), 3);
    }

    #[test]
    fn page_keys_move_by_the_visible_row_count() {
        let mut form = SearchSelect::new("Pick a book", books());
        form.visible_rows = 1;

        form.handle_key(key(KeyCode::PageDown));
        assert_eq!(form.selected, 1);

        form.handle_key(key(KeyCode::PageDown));
        assert_eq!(form.selected, 2);

        form.handle_key(key(KeyCode::PageDown));
        assert_eq!(form.selected, 2);

        form.handle_key(key(KeyCode::PageUp));
        assert_eq!(form.selected, 1);
    }

    #[test]
    fn arrows_clamp_at_the_ends() {
        let mut form = SearchSelect::new("Pick a book", books());

        form.handle_key(key(KeyCode::Up));
        assert_eq!(form.selected, 0);

        form.handle_key(key(KeyCode::Down));
        form.handle_key(key(KeyCode::Down));
        form.handle_key(key(KeyCode::Down));
        assert_eq!(form.selected, 2);
    }

    #[test]
    fn home_and_end_edit_the_query_unless_ctrl_is_held() {
        let mut form = SearchSelect::new("Pick a book", books());
        form.input.insert_str("abc");
        form.input.home();

        form.handle_key(key(KeyCode::End));
        assert_eq!(form.input.cursor, 3);

        form.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL));
        assert_eq!(form.selected, 0);

        form.handle_key(key(KeyCode::Home));
        assert_eq!(form.input.cursor, 0);
    }
}
