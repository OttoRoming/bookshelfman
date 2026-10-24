//! A searchable single-select form rendered inline.
//!
//! [`SearchSelect`] takes a prompt and either a fixed list of [`SelectItem`]s
//! or a search callback, and returns the selected item. It is similar in
//! spirit to `inquire`'s select prompt or Go's `huh` select field, with one
//! important difference: the form renders at the current cursor row and
//! **never enters the alternate screen**. Existing terminal output stays
//! visible above the form.
//!
//! With a fixed list, typing filters the list locally. With a search callback,
//! typing starts a debounced search on a worker thread and the form shows an
//! animated spinner until the results arrive. Callback errors are displayed in
//! the results pane.
//!
//! Items may expose [`Metadata`] (shown in a preview pane and included in
//! local search matching), and a thumbnail callback can provide an arbitrary
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
//! To load items on demand, use [`SearchSelect::search`] or
//! [`SearchSelect::with_search`]. The callback runs on a worker thread, so it
//! may block (for example on network or disk I/O):
//!
//! ```no_run
//! use bookshelfman::forms::SearchSelect;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let selected = SearchSelect::search("Search", |query| {
//!     Ok(vec![format!("result for {query}")])
//! })
//! .with_initial_query("gatsby")
//! .run()?;
//! # let _ = selected;
//! # Ok(())
//! # }
//! ```
//!
//! # Controls
//!
//! | Key | Action |
//! | --- | --- |
//! | Type | Filter the list, or run the search callback |
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
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Position;
use ratatui::widgets::ListState;

use state::{EntryInfo, Input, moved_selection, rank_matches};
use terminal::Session;

/// The default number of result rows.
const DEFAULT_PAGE_SIZE: usize = 8;
/// The largest number of result rows that can be requested.
const MAX_PAGE_SIZE: usize = 50;

/// How long to wait after a query change before invoking the search callback.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(250);
/// How often to poll for search results and advance the spinner.
const SEARCH_TICK: Duration = Duration::from_millis(80);
/// How long an idle form waits for input.
const IDLE_POLL: Duration = Duration::from_secs(3_600);

/// Frames for the search spinner.
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

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

/// An error returned by a search callback.
///
/// Errors are displayed in the form's result area, and the user can keep
/// editing the query to retry.
pub type SearchError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// How the event loop finished.
enum Outcome {
    Selected(usize),
    Cancelled,
}

/// Builds a thumbnail for an item. See [`SearchSelect::with_thumbnail`].
type ThumbnailCallback<'a, T> = Box<dyn Fn(&T) -> Option<Thumbnail> + 'a>;

/// The query and result sent back from a search worker.
type SearchMessage<T> = (String, std::result::Result<Vec<T>, SearchError>);

/// Spawns a search worker for a query and returns its result channel.
type SearchSpawner<T> = Arc<dyn Fn(String) -> Receiver<SearchMessage<T>> + Send + Sync>;

/// An inline searchable select form.
///
/// Build the form with [`SearchSelect::new`] for a fixed list, or with
/// [`SearchSelect::search`] to have a callback provide results for the current
/// query. A thumbnail callback and other options can then be attached before
/// calling [`run`](SearchSelect::run).
pub struct SearchSelect<'a, T> {
    prompt: String,
    items: Vec<T>,
    entries: Vec<EntryInfo>,
    thumbnail: Option<ThumbnailCallback<'a, T>>,
    thumbnails: HashMap<usize, Option<Thumbnail>>,
    search_spawner: Option<SearchSpawner<T>>,
    pending_search: Option<Receiver<SearchMessage<T>>>,
    search_due: Option<Instant>,
    search_error: Option<String>,
    spinner_frame: usize,
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
            search_spawner: None,
            pending_search: None,
            search_due: None,
            search_error: None,
            spinner_frame: 0,
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

    /// Creates a form whose items are supplied by a search callback.
    ///
    /// The callback takes the current query, runs on a worker thread, and
    /// returns the matching items. It is invoked after a short debounce and is
    /// not called for empty queries. Errors are shown in the form.
    ///
    /// Use [`with_initial_query`](SearchSelect::with_initial_query) to start
    /// with a query and an immediate search.
    ///
    /// ```no_run
    /// use bookshelfman::forms::SearchSelect;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let selected = SearchSelect::search("Search", |query| {
    ///     Ok(vec![format!("result for {query}")])
    /// })
    /// .with_initial_query("gatsby")
    /// .run()?;
    /// # let _ = selected;
    /// # Ok(())
    /// # }
    /// ```
    pub fn search(
        prompt: impl Into<String>,
        callback: impl Fn(String) -> std::result::Result<Vec<T>, SearchError> + Send + Sync + 'static,
    ) -> Self
    where
        T: Send + 'static,
    {
        Self::new(prompt, std::iter::empty::<T>()).with_search(callback)
    }

    /// Sets a callback that loads the items for the current query.
    ///
    /// The callback runs on a worker thread, so it may block (for example on
    /// network or disk I/O). Queries are debounced by 250 ms and stale
    /// responses are discarded. A spinner is shown while a search is pending.
    ///
    /// Async clients can be driven from the worker thread, e.g. with a
    /// `tokio::runtime::Handle` captured outside the callback.
    pub fn with_search(
        mut self,
        callback: impl Fn(String) -> std::result::Result<Vec<T>, SearchError> + Send + Sync + 'static,
    ) -> Self
    where
        T: Send + 'static,
    {
        let callback: Arc<
            dyn Fn(String) -> std::result::Result<Vec<T>, SearchError> + Send + Sync,
        > = Arc::new(callback);

        let spawner: SearchSpawner<T> = Arc::new(move |query: String| {
            let (sender, receiver) = mpsc::channel();
            let thread_sender = sender.clone();
            let thread_query = query.clone();
            let callback = Arc::clone(&callback);

            let spawned = std::thread::Builder::new()
                .name("bookshelfman-search".to_string())
                .spawn(move || {
                    let result = callback(thread_query.clone());
                    let _ = thread_sender.send((thread_query, result));
                });

            if let Err(error) = spawned {
                let _ = sender.send((query, Err(Box::new(error))));
            }

            receiver
        });

        self.search_spawner = Some(spawner);
        self.pending_search = None;
        self.search_error = None;
        self.spinner_frame = 0;
        self.search_due = if self.input.value.trim().is_empty() {
            None
        } else {
            Some(Instant::now())
        };

        self
    }

    /// Sets the initial query.
    ///
    /// With a search callback attached, this starts a search immediately. With
    /// a fixed list, the list is filtered by the query.
    pub fn with_initial_query(mut self, query: impl Into<String>) -> Self {
        self.input.set_value(query.into());

        if self.search_spawner.is_some() {
            self.clear_search_results();
            self.search_due = if self.input.value.trim().is_empty() {
                None
            } else {
                Some(Instant::now())
            };
        } else {
            self.refilter();
        }

        self
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

    /// Sets the message shown by a fixed-list form when the list is empty.
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
        let mut dirty = true;

        loop {
            if self.tick() {
                dirty = true;
            }

            if dirty {
                session.draw(|frame| self.render(frame))?;
                dirty = false;
            }

            let timeout = if self.is_searching() {
                SEARCH_TICK
            } else {
                IDLE_POLL
            };

            if !event::poll(timeout)? {
                continue;
            }

            let event = event::read()?;
            dirty = true;

            match event {
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
                    self.on_query_changed();
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }

    /// Advances search work and the spinner. Returns whether the form changed.
    fn tick(&mut self) -> bool {
        let mut changed = false;

        if let Some(receiver) = self.pending_search.take() {
            match receiver.try_recv() {
                Ok((query, result)) => {
                    if query == self.input.value {
                        self.apply_search_result(result);
                    }

                    changed = true;
                }
                Err(TryRecvError::Empty) => {
                    self.pending_search = Some(receiver);
                }
                Err(TryRecvError::Disconnected) => {
                    self.search_error = Some("search worker stopped unexpectedly".to_string());
                    changed = true;
                }
            }
        }

        if let Some(due) = self.search_due
            && Instant::now() >= due
            && self.pending_search.is_none()
        {
            self.search_due = None;
            self.start_search();
            changed = true;
        }

        if self.is_searching() {
            self.spinner_frame = self.spinner_frame.wrapping_add(1);
            changed = true;
        }

        changed
    }

    fn start_search(&mut self) {
        let Some(spawner) = self.search_spawner.as_ref() else {
            return;
        };

        self.search_error = None;
        self.pending_search = Some(spawner(self.input.value.clone()));
    }

    fn apply_search_result(&mut self, result: std::result::Result<Vec<T>, SearchError>) {
        match result {
            Ok(items) => {
                self.items = items;
                self.entries = self
                    .items
                    .iter()
                    .map(|item| EntryInfo::new(item))
                    .collect::<Vec<_>>();
                self.has_metadata = self.entries.iter().any(|entry| !entry.metadata.is_empty());
                self.matches = (0..self.items.len()).collect::<Vec<_>>();
                self.selected = 0;
                *self.list_state.offset_mut() = 0;
                self.list_state
                    .select((!self.matches.is_empty()).then_some(0));
                self.thumbnails.clear();
                self.search_error = None;
            }
            Err(error) => {
                self.items.clear();
                self.entries.clear();
                self.matches.clear();
                self.selected = 0;
                *self.list_state.offset_mut() = 0;
                self.list_state.select(None);
                self.thumbnails.clear();
                self.has_metadata = false;
                self.search_error = Some(error.to_string());
            }
        }
    }

    fn clear_search_results(&mut self) {
        self.items.clear();
        self.entries.clear();
        self.matches.clear();
        self.selected = 0;
        *self.list_state.offset_mut() = 0;
        self.list_state.select(None);
        self.thumbnails.clear();
        self.pending_search = None;
        self.search_due = None;
        self.search_error = None;
        self.spinner_frame = 0;
        self.has_metadata = false;
    }

    fn is_searching(&self) -> bool {
        self.search_spawner.is_some()
            && (self.search_due.is_some() || self.pending_search.is_some())
    }

    fn spinner(&self) -> &'static str {
        SPINNER[self.spinner_frame % SPINNER.len()]
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
                    self.on_query_changed();
                }
            }
            KeyCode::Backspace => {
                if self.input.backspace() {
                    self.on_query_changed();
                }
            }
            KeyCode::Delete => {
                if self.input.delete() {
                    self.on_query_changed();
                }
            }
            KeyCode::Char('u') if control => {
                self.input.clear();
                self.on_query_changed();
            }
            KeyCode::Char('w') if control => {
                if self.input.delete_word() {
                    self.on_query_changed();
                }
            }
            KeyCode::Char('a') if control => self.input.home(),
            KeyCode::Char('e') if control => self.input.end(),
            KeyCode::Char(ch) if !control && !key.modifiers.contains(KeyModifiers::ALT) => {
                self.input.insert_char(ch);
                self.on_query_changed();
            }
            _ => {}
        }

        None
    }

    fn on_query_changed(&mut self) {
        if self.search_spawner.is_some() {
            self.clear_search_results();

            if self.input.value.trim().is_empty() {
                self.search_due = None;
            } else {
                self.search_due = Some(Instant::now() + SEARCH_DEBOUNCE);
            }

            self.spinner_frame = 0;
        } else {
            self.refilter();
        }
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

    #[test]
    fn search_callback_provides_results() {
        let mut form =
            SearchSelect::search("Search", |query| Ok(vec![format!("result for {query}")]));

        form.input.set_value("gatsby");
        form.on_query_changed();
        form.search_due = Some(Instant::now());

        let mut loaded = false;

        for _ in 0..200 {
            form.tick();

            if form.items == vec!["result for gatsby".to_string()] {
                loaded = true;
                break;
            }

            std::thread::sleep(Duration::from_millis(5));
        }

        assert!(loaded);
        assert_eq!(form.matches, vec![0]);
        assert!(form.search_error.is_none());
        assert!(!form.is_searching());
    }

    #[test]
    fn search_errors_are_reported() {
        let mut form = SearchSelect::search(
            "Search",
            |_| -> std::result::Result<Vec<String>, SearchError> {
                Err(Box::new(std::io::Error::other("boom")))
            },
        );

        form.input.set_value("gatsby");
        form.on_query_changed();
        form.search_due = Some(Instant::now());

        for _ in 0..200 {
            form.tick();

            if form.search_error.is_some() {
                break;
            }

            std::thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(form.search_error.as_deref(), Some("boom"));
        assert!(form.matches.is_empty());
        assert!(!form.is_searching());
    }

    #[test]
    fn empty_queries_do_not_start_a_search() {
        let mut form = SearchSelect::search("Search", |_| Ok(vec!["result".to_string()]));

        form.input.set_value("");
        form.on_query_changed();

        assert!(!form.is_searching());
    }

    #[test]
    fn an_initial_query_starts_the_search_immediately() {
        let form = SearchSelect::search("Search", |_| Ok(vec!["result".to_string()]))
            .with_initial_query("gatsby");

        assert!(form.search_due.is_some());
        assert!(form.is_searching());
    }
}
