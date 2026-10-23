//! Interactive terminal forms.
//!
//! Forms are rendered inline at the current cursor position. They enable raw
//! mode for keyboard input, restore it on exit, and never enter the alternate
//! screen, so output printed before the form stays on screen.
//!
//! The currently available form is [`search_select::SearchSelect`], a
//! searchable single-select field.

pub mod search_select;

pub use search_select::{Metadata, SearchSelect, SelectItem, Thumbnail};
