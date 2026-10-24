//! Interactive terminal forms.
//!
//! Forms are rendered inline at the current cursor position. They enable raw
//! mode for keyboard input, restore it on exit, and never enter the alternate
//! screen, so output printed before the form stays on screen.
//!
//! The currently available form is [`search_select::SearchSelect`], a
//! searchable single-select field. It can filter a fixed list locally or load
//! results from a callback, showing a spinner while a search is in flight.

pub mod search_select;

pub use search_select::{Metadata, SearchError, SearchSelect, SelectItem, Thumbnail};
