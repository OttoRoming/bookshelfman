//! Search, filtering, and single-line input state.
//!
//! Everything in this module is rendering-independent so it can be unit tested
//! without a terminal.

use unicode_width::UnicodeWidthChar;

use super::item::{Metadata, SelectItem};

/// A display-friendly, searchable snapshot of one item.
///
/// The item itself stays owned by [`SearchSelect`](super::SearchSelect); this
/// keeps the label and metadata available without asking the item for them on
/// every frame or keystroke.
#[derive(Debug, Clone, Default)]
pub(crate) struct EntryInfo {
    pub(crate) label: String,
    pub(crate) metadata: Vec<Metadata>,
    pieces: Vec<String>,
}

impl EntryInfo {
    pub(crate) fn new<T: SelectItem + ?Sized>(item: &T) -> Self {
        let label = single_line(item.label());
        let metadata = item
            .metadata()
            .iter()
            .map(|metadata| Metadata {
                label: single_line(&metadata.label),
                value: single_line(&metadata.value),
            })
            .collect::<Vec<_>>();

        let mut pieces = Vec::with_capacity(1 + metadata.len() * 2);
        pieces.push(label.to_lowercase());

        for metadata in &metadata {
            pieces.push(metadata.label.to_lowercase());
            pieces.push(metadata.value.to_lowercase());
        }

        Self {
            label,
            metadata,
            pieces,
        }
    }
}

/// Replaces control characters so a value always occupies exactly one line.
fn single_line(text: &str) -> String {
    text.chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect()
}

/// Returns the indices of the entries matching `query`, best match first.
///
/// An empty (or whitespace-only) query returns every entry in its original
/// order. Otherwise every whitespace-separated term must match either the
/// label or one of the metadata fields, using a case-insensitive substring
/// match with a subsequence fallback. Ties keep their original order.
pub(crate) fn rank_matches(entries: &[EntryInfo], query: &str) -> Vec<usize> {
    let terms = query
        .split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>();

    if terms.is_empty() {
        return (0..entries.len()).collect();
    }

    let mut matches = Vec::new();

    for (index, entry) in entries.iter().enumerate() {
        let mut total = 0;
        let mut matched_all = true;

        for term in &terms {
            let best = entry
                .pieces
                .iter()
                .filter_map(|piece| score_term(term, piece))
                .max();

            match best {
                Some(score) => total += score,
                None => {
                    matched_all = false;
                    break;
                }
            }
        }

        if matched_all {
            matches.push((total, index));
        }
    }

    matches.sort_by(|(score_a, index_a), (score_b, index_b)| {
        score_b.cmp(score_a).then_with(|| index_a.cmp(index_b))
    });

    matches.into_iter().map(|(_, index)| index).collect()
}

/// Scores a single query term against one searchable field.
fn score_term(term: &str, piece: &str) -> Option<u32> {
    if term.is_empty() {
        return Some(0);
    }

    if piece == term {
        return Some(1_000);
    }

    if piece.starts_with(term) {
        return Some(800);
    }

    if let Some(position) = piece.find(term) {
        let on_word_boundary = piece[..position]
            .chars()
            .next_back()
            .is_none_or(|ch| !ch.is_alphanumeric());
        let base: u32 = if on_word_boundary { 600 } else { 400 };

        return Some(base.saturating_sub(position.min(200) as u32));
    }

    fuzzy_score(term, piece)
}

/// Scores a subsequence match, preferring terms matched close together.
fn fuzzy_score(term: &str, piece: &str) -> Option<u32> {
    let mut term_chars = term.chars();
    let mut wanted = term_chars.next()?;
    let mut matched = false;
    let mut total_gap = 0;

    for ch in piece.chars() {
        if ch == wanted {
            matched = true;

            match term_chars.next() {
                Some(next) => wanted = next,
                None => return Some(300u32.saturating_sub(total_gap.min(299))),
            }
        } else if matched {
            total_gap += 1;
        }
    }

    None
}

/// Returns the next selection when moving by `delta`, clamped to the list.
pub(crate) fn moved_selection(selected: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }

    let last = len - 1;

    if delta < 0 {
        selected.saturating_sub(delta.unsigned_abs())
    } else {
        selected.saturating_add(delta as usize).min(last)
    }
}

/// A single-line text input with a byte-indexed cursor.
#[derive(Debug, Clone, Default)]
pub(crate) struct Input {
    pub(crate) value: String,
    /// A byte offset into `value`, always on a character boundary.
    pub(crate) cursor: usize,
}

impl Input {
    pub(crate) fn insert_char(&mut self, ch: char) {
        self.value.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    /// Inserts pasted text, flattening any line breaks or control characters.
    pub(crate) fn insert_str(&mut self, text: &str) {
        for ch in text.chars() {
            self.insert_char(if ch.is_control() { ' ' } else { ch });
        }
    }

    pub(crate) fn backspace(&mut self) -> bool {
        let Some(previous) = self.previous_boundary() else {
            return false;
        };

        self.value.replace_range(previous..self.cursor, "");
        self.cursor = previous;

        true
    }

    pub(crate) fn delete(&mut self) -> bool {
        if self.cursor >= self.value.len() {
            return false;
        }

        let next = self.next_boundary();
        self.value.replace_range(self.cursor..next, "");

        true
    }

    pub(crate) fn delete_word(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }

        let before = &self.value[..self.cursor];
        let mut start = self.cursor;
        let mut in_word = false;

        for (index, ch) in before.char_indices().rev() {
            if ch.is_whitespace() {
                if in_word {
                    start = index + ch.len_utf8();
                    break;
                }

                start = index;
            } else {
                in_word = true;
                start = index;
            }
        }

        if !in_word {
            start = 0;
        }

        if start == self.cursor {
            return false;
        }

        self.value.replace_range(start..self.cursor, "");
        self.cursor = start;

        true
    }

    pub(crate) fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    /// Replaces the value and moves the cursor to the end.
    pub(crate) fn set_value(&mut self, value: impl Into<String>) {
        self.value.clear();
        self.cursor = 0;

        let value = value.into();
        self.insert_str(&value);
    }

    pub(crate) fn move_left(&mut self) {
        if let Some(previous) = self.previous_boundary() {
            self.cursor = previous;
        }
    }

    pub(crate) fn move_right(&mut self) {
        if self.cursor < self.value.len() {
            self.cursor = self.next_boundary();
        }
    }

    pub(crate) fn home(&mut self) {
        self.cursor = 0;
    }

    pub(crate) fn end(&mut self) {
        self.cursor = self.value.len();
    }

    fn previous_boundary(&self) -> Option<usize> {
        if self.cursor == 0 {
            return None;
        }

        let mut index = self.cursor - 1;

        while !self.value.is_char_boundary(index) {
            index -= 1;
        }

        Some(index)
    }

    fn next_boundary(&self) -> usize {
        let mut index = self.cursor + 1;

        while index < self.value.len() && !self.value.is_char_boundary(index) {
            index += 1;
        }

        index
    }
}

/// Returns the part of the query visible in `width` columns and the cursor's
/// column offset within that part.
///
/// The returned text scrolls horizontally to keep the cursor inside the input
/// area. `cursor` must be a character boundary in `value`.
pub(crate) fn visible_query(value: &str, cursor: usize, width: usize) -> (String, usize) {
    if width == 0 {
        return (String::new(), 0);
    }

    let cursor = cursor.min(value.len());
    let char_width = |ch: char| UnicodeWidthChar::width(ch).unwrap_or(0);

    let mut start = 0;
    let mut cursor_width = 0;

    for (_index, ch) in value[..cursor].char_indices() {
        cursor_width += char_width(ch);

        while cursor_width >= width {
            let Some(first) = value[start..].chars().next() else {
                break;
            };

            let first_width = char_width(first);
            start += first.len_utf8();
            cursor_width = cursor_width.saturating_sub(first_width);
        }
    }

    let mut visible = String::new();
    let mut used = 0;
    let mut cursor_offset = 0;

    for (index, ch) in value[start..].char_indices() {
        let width_of_char = char_width(ch);

        if used + width_of_char > width {
            break;
        }

        if start + index == cursor {
            cursor_offset = used;
        }

        visible.push(ch);
        used += width_of_char;
    }

    if cursor == value.len() {
        cursor_offset = used;
    }

    (visible, cursor_offset)
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

    fn entries() -> Vec<EntryInfo> {
        [
            Book::new("The Great Gatsby", "F. Scott Fitzgerald"),
            Book::new("The Dispossessed", "Ursula K. Le Guin"),
            Book::new("Kindred", "Octavia E. Butler"),
            Book::new("Great Expectations", "Charles Dickens"),
        ]
        .iter()
        .map(EntryInfo::new)
        .collect()
    }

    fn labels<'a>(entries: &'a [EntryInfo], matches: &[usize]) -> Vec<&'a str> {
        matches
            .iter()
            .map(|&index| entries[index].label.as_str())
            .collect()
    }

    #[test]
    fn empty_query_returns_everything_in_order() {
        let entries = entries();
        let matches = rank_matches(&entries, "");

        assert_eq!(matches, vec![0, 1, 2, 3]);
        assert_eq!(rank_matches(&entries, "   "), vec![0, 1, 2, 3]);
    }

    #[test]
    fn matches_labels_case_insensitively() {
        let entries = entries();
        let matches = rank_matches(&entries, "GATSBY");

        assert_eq!(labels(&entries, &matches), vec!["The Great Gatsby"]);
    }

    #[test]
    fn matches_metadata_values() {
        let entries = entries();
        let matches = rank_matches(&entries, "ursula");

        assert_eq!(labels(&entries, &matches), vec!["The Dispossessed"]);
    }

    #[test]
    fn requires_every_term_to_match() {
        let entries = entries();
        let matches = rank_matches(&entries, "great fitzgerald");

        assert_eq!(labels(&entries, &matches), vec!["The Great Gatsby"]);
    }

    #[test]
    fn ranks_prefixes_above_later_substrings() {
        let entries = entries();
        let matches = rank_matches(&entries, "great");

        assert_eq!(
            labels(&entries, &matches),
            vec!["Great Expectations", "The Great Gatsby"]
        );
    }

    #[test]
    fn fuzzy_matches_are_supported() {
        let entries = entries();
        let matches = rank_matches(&entries, "tgg");

        assert_eq!(labels(&entries, &matches), vec!["The Great Gatsby"]);
    }

    #[test]
    fn unknown_queries_match_nothing() {
        let entries = entries();

        assert!(rank_matches(&entries, "zzzzz").is_empty());
    }

    #[test]
    fn control_characters_are_flattened() {
        let entry = EntryInfo::new(&Book::new("Title\nWith\tTabs", "Author\rName"));

        assert_eq!(entry.label, "Title With Tabs");
        assert_eq!(entry.metadata[0].value, "Author Name");
    }

    #[test]
    fn selection_moves_and_clamps() {
        assert_eq!(moved_selection(0, 4, -1), 0);
        assert_eq!(moved_selection(0, 4, 1), 1);
        assert_eq!(moved_selection(3, 4, 1), 3);
        assert_eq!(moved_selection(1, 4, -2), 0);
        assert_eq!(moved_selection(0, 0, 3), 0);
    }

    #[test]
    fn input_edits_and_moves_by_character() {
        let mut input = Input::default();
        input.insert_str("héllo");

        assert_eq!(input.value, "héllo");
        assert_eq!(input.cursor, 6);

        assert!(input.backspace());
        assert_eq!(input.value, "héll");

        input.home();
        assert!(input.delete());
        assert_eq!(input.value, "éll");

        input.end();
        input.move_left();
        input.insert_char('!');
        assert_eq!(input.value, "él!l");
    }

    #[test]
    fn deleting_a_word_handles_trailing_space() {
        let mut input = Input::default();
        input.insert_str("foo bar ");

        assert!(input.delete_word());
        assert_eq!(input.value, "foo ");
        assert!(input.delete_word());
        assert_eq!(input.value, "");
        assert!(!input.delete_word());
    }

    #[test]
    fn pasted_control_characters_become_spaces() {
        let mut input = Input::default();
        input.insert_str("one\ntwo\tthree");

        assert_eq!(input.value, "one two three");
    }

    #[test]
    fn setting_a_value_moves_the_cursor_to_the_end() {
        let mut input = Input::default();
        input.set_value("the great gatsby");

        assert_eq!(input.value, "the great gatsby");
        assert_eq!(input.cursor, input.value.len());
    }

    #[test]
    fn cursor_stays_inside_the_input_width() {
        let (visible, cursor) = visible_query("abcdef", 6, 5);

        assert_eq!(visible, "cdef");
        assert_eq!(cursor, 4);
    }

    #[test]
    fn no_scroll_is_needed_for_short_queries() {
        let (visible, cursor) = visible_query("abc", 3, 10);

        assert_eq!(visible, "abc");
        assert_eq!(cursor, 3);
    }

    #[test]
    fn cursor_can_be_in_the_middle() {
        let (visible, cursor) = visible_query("abcdef", 2, 10);

        assert_eq!(visible, "abcdef");
        assert_eq!(cursor, 2);
    }

    #[test]
    fn wide_characters_use_two_columns() {
        let (visible, cursor) = visible_query("書籍", 6, 3);

        assert_eq!(visible, "籍");
        assert_eq!(cursor, 2);
    }

    #[test]
    fn zero_width_does_not_panic() {
        assert_eq!(visible_query("abc", 0, 0), (String::new(), 0));
    }
}
