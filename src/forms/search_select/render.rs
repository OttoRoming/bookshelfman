//! Rendering for [`SearchSelect`].

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, HighlightSpacing, List, ListItem, Paragraph};
use unicode_width::UnicodeWidthStr;

use super::SearchSelect;
use super::item::SelectItem;
use super::state::{EntryInfo, visible_query};
use super::thumbnail::Thumbnail;

const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;
const PREVIEW_MIN_WIDTH: u16 = 40;
const HELP: &str = "↑/↓ move  enter select  ctrl+u clear  esc cancel";

impl<T: SelectItem> SearchSelect<'_, T> {
    pub(super) fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        self.cursor_pos = None;

        if area.is_empty() {
            return;
        }

        self.ensure_selected_thumbnail();

        let [header, body, footer] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);

        self.render_header(frame, header);
        self.render_body(frame, body);
        self.render_footer(frame, footer);

        if let Some(position) = self.cursor_pos {
            frame.set_cursor_position(position);
        }
    }

    fn render_header(&mut self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        self.render_prompt(frame, area);

        if area.height >= 2 {
            self.render_input(frame, Rect::new(area.x, area.y + 1, area.width, 1));
        }
    }

    fn render_prompt(&self, frame: &mut Frame, area: Rect) {
        let selected = self.selected.min(self.matches.len().saturating_sub(1));
        let count = if self.matches.is_empty() {
            "0/0".to_string()
        } else {
            format!("{}/{}", selected + 1, self.matches.len())
        };
        let count_width = u16::try_from(UnicodeWidthStr::width(count.as_str())).unwrap_or(u16::MAX);

        if count_width <= area.width {
            let count_area = Rect::new(area.right() - count_width, area.y, count_width, 1);
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(count, muted()))).right_aligned(),
                count_area,
            );
        }

        let prompt_width = area.width.saturating_sub(count_width).saturating_sub(2);

        if prompt_width == 0 {
            return;
        }

        let line = Line::from(vec![
            Span::styled("? ", Style::new().fg(ACCENT).add_modifier(Modifier::BOLD)),
            Span::styled(
                self.prompt.clone(),
                Style::new().add_modifier(Modifier::BOLD),
            ),
        ]);
        let prompt_area = Rect::new(area.x, area.y, prompt_width, 1);

        frame.render_widget(Paragraph::new(line), prompt_area);
    }

    fn render_input(&mut self, frame: &mut Frame, area: Rect) {
        if area.width <= 2 {
            return;
        }

        let available = usize::from(area.width - 2);
        let (visible, cursor_offset) =
            visible_query(&self.input.value, self.input.cursor, available);

        let mut spans = vec![Span::styled("> ", Style::new().fg(ACCENT))];

        if self.input.value.is_empty() {
            // Keep a cell free for the terminal cursor before the placeholder.
            spans.push(Span::raw(" "));
            spans.push(Span::styled("Type to search…", muted()));
        } else {
            spans.push(Span::raw(visible));
        }

        frame.render_widget(Paragraph::new(Line::from(spans)), area);

        let offset = u16::try_from(cursor_offset).unwrap_or(u16::MAX);
        let x = area
            .x
            .saturating_add(2)
            .saturating_add(offset)
            .min(area.right().saturating_sub(1));

        self.cursor_pos = Some(Position::new(x, area.y));
    }

    fn render_body(&mut self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            self.visible_rows = 0;
            return;
        }

        let show_preview =
            area.width >= PREVIEW_MIN_WIDTH && (self.thumbnail.is_some() || self.has_metadata);

        if show_preview {
            let [list_area, preview_area] =
                Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)])
                    .spacing(1)
                    .areas(area);

            self.render_list(frame, list_area);
            self.render_preview(frame, preview_area);
        } else {
            self.render_list(frame, area);
        }
    }

    fn render_list(&mut self, frame: &mut Frame, area: Rect) {
        let block = panel("Results");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        self.visible_rows = usize::from(inner.height);

        if inner.is_empty() {
            return;
        }

        if self.matches.is_empty() {
            self.list_state.select(None);
            render_centered_line(frame, inner, &self.empty_message_text(), muted());

            return;
        }

        self.selected = self.selected.min(self.matches.len() - 1);
        self.list_state.select(Some(self.selected));

        let items = self
            .matches
            .iter()
            .map(|&index| ListItem::new(Line::from(Span::raw(self.entries[index].label.clone()))));

        let list = List::new(items)
            .highlight_symbol(Line::from(Span::styled(
                "> ",
                Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
            )))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD))
            .highlight_spacing(HighlightSpacing::Always);

        frame.render_stateful_widget(list, inner, &mut self.list_state);
    }

    fn empty_message_text(&self) -> String {
        let query = self.input.value.trim();

        if query.is_empty() {
            if self.entries.is_empty() {
                "No options".to_string()
            } else {
                self.empty_message.clone()
            }
        } else {
            format!("No matches for \"{query}\"")
        }
    }

    fn render_preview(&self, frame: &mut Frame, area: Rect) {
        let block = panel("Preview");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.is_empty() {
            return;
        }

        let Some(&entry_index) = self.matches.get(self.selected) else {
            render_centered_line(frame, inner, "No selection", muted());

            return;
        };

        let info = &self.entries[entry_index];
        let thumbnail = self.thumbnails.get(&entry_index).and_then(Option::as_ref);
        let want_thumbnail = self.thumbnail.is_some();
        let metadata_count = info.metadata.len();

        let (thumbnail_area, metadata_area) = if want_thumbnail && metadata_count > 0 {
            let max_metadata_rows = (usize::from(inner.height) / 2).max(1);
            let metadata_rows =
                u16::try_from(metadata_count.min(max_metadata_rows)).unwrap_or(inner.height);
            let thumbnail_height = inner.height.saturating_sub(metadata_rows);
            let [thumbnail_area, metadata_area] = Layout::vertical([
                Constraint::Length(thumbnail_height),
                Constraint::Length(metadata_rows),
            ])
            .areas(inner);

            (Some(thumbnail_area), Some(metadata_area))
        } else if want_thumbnail {
            (Some(inner), None)
        } else if metadata_count > 0 {
            (None, Some(inner))
        } else {
            (None, None)
        };

        if let Some(area) = thumbnail_area {
            render_thumbnail(frame, area, thumbnail);
        }

        if let Some(area) = metadata_area {
            let lines = metadata_lines(info, usize::from(area.height));
            frame.render_widget(Paragraph::new(Text::from(lines)), area);
        }

        if thumbnail_area.is_none() && metadata_area.is_none() {
            render_centered_line(frame, inner, "No details", muted());
        }
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let help = self.help.as_deref().unwrap_or(HELP);

        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(help.to_string(), muted()))),
            area,
        );
    }
}

fn panel(title: &'static str) -> Block<'static> {
    Block::bordered()
        .border_style(muted())
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(title, muted().add_modifier(Modifier::BOLD)),
            Span::raw(" "),
        ]))
}

fn render_thumbnail(frame: &mut Frame, area: Rect, thumbnail: Option<&Thumbnail>) {
    if area.is_empty() {
        return;
    }

    match thumbnail {
        Some(thumbnail) if !thumbnail.is_empty() => {
            let width = thumbnail.width().min(area.width);
            let height = thumbnail.height().min(area.height);

            if width == 0 || height == 0 {
                return;
            }

            let x = area.x + (area.width - width) / 2;
            let y = area.y + (area.height - height) / 2;
            let target = Rect::new(x, y, width, height);

            frame.render_widget(
                Paragraph::new(Text::from(thumbnail.lines().to_vec())),
                target,
            );
        }
        _ => render_centered_line(frame, area, "(no preview)", muted()),
    }
}

fn metadata_lines(info: &EntryInfo, max_rows: usize) -> Vec<Line<'static>> {
    let total = info.metadata.len();

    if total == 0 || max_rows == 0 {
        return Vec::new();
    }

    let visible = total.min(max_rows);
    let truncated = total > visible && visible >= 2;
    let shown = if truncated { visible - 1 } else { visible };
    let mut lines = Vec::with_capacity(visible);

    for metadata in info.metadata.iter().take(shown) {
        lines.push(Line::from(vec![
            Span::styled(format!("{}: ", metadata.label), muted()),
            Span::raw(metadata.value.clone()),
        ]));
    }

    if truncated {
        lines.push(Line::from(Span::styled(
            format!("+{} more", total - shown),
            muted().add_modifier(Modifier::DIM),
        )));
    }

    lines
}

fn render_centered_line(frame: &mut Frame, area: Rect, text: &str, style: Style) {
    if area.is_empty() {
        return;
    }

    let target = Rect::new(area.x, area.y + area.height / 2, area.width, 1);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(text.to_string(), style))).centered(),
        target,
    );
}

fn muted() -> Style {
    Style::new().fg(MUTED)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::forms::search_select::item::Metadata;
    use crate::forms::search_select::{SearchSelect, SelectItem};

    struct Book {
        title: String,
        metadata: Vec<Metadata>,
        cover: String,
    }

    impl Book {
        fn new(title: &str, author: &str, cover: &str) -> Self {
            Self {
                title: title.to_string(),
                metadata: vec![Metadata::new("Author", author)],
                cover: cover.to_string(),
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
            Book::new("The Dispossessed", "Ursula K. Le Guin", "DISP"),
            Book::new("The Great Gatsby", "F. Scott Fitzgerald", "GATS"),
        ]
    }

    fn render_text(form: &mut SearchSelect<'_, Book>, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| form.render(frame)).unwrap();

        buffer_text(terminal.backend().buffer())
    }

    fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
        let mut text = String::new();

        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                text.push_str(buffer[(x, y)].symbol());
            }

            text.push('\n');
        }

        text
    }

    #[test]
    fn renders_prompt_items_and_metadata() {
        let mut form = SearchSelect::new("Pick a book", books());

        let text = render_text(&mut form, 90, 20);

        assert!(text.contains("Pick a book"));
        assert!(text.contains("1/2"));
        assert!(text.contains("> The Dispossessed"));
        assert!(text.contains("The Great Gatsby"));
        assert!(text.contains("Author: Ursula K. Le Guin"));
        assert!(text.contains("Type to search…"));
    }

    #[test]
    fn filters_as_the_query_changes() {
        let mut form = SearchSelect::new("Pick a book", books());
        form.input.insert_str("ursula");
        form.refilter();

        let text = render_text(&mut form, 90, 20);

        assert!(text.contains("> ursula"));
        assert!(text.contains("The Dispossessed"));
        assert!(!text.contains("The Great Gatsby"));
        assert!(text.contains("1/1"));
    }

    #[test]
    fn shows_the_empty_state() {
        let mut form = SearchSelect::new("Pick a book", books());
        form.input.insert_str("zzz");
        form.refilter();

        let text = render_text(&mut form, 90, 20);

        assert!(text.contains("No matches for \"zzz\""));
        assert!(text.contains("0/0"));
    }

    #[test]
    fn renders_and_caches_thumbnails() {
        let calls = Rc::new(Cell::new(0));
        let callback_calls = Rc::clone(&calls);
        let mut form = SearchSelect::new("Pick a book", books()).with_thumbnail(move |book| {
            callback_calls.set(callback_calls.get() + 1);
            Some(Thumbnail::from_text(&book.cover))
        });

        let text = render_text(&mut form, 90, 20);
        assert!(text.contains("DISP"));
        assert_eq!(calls.get(), 1);

        let text = render_text(&mut form, 90, 20);
        assert!(text.contains("DISP"));
        assert_eq!(calls.get(), 1);

        form.move_selection(1);
        let text = render_text(&mut form, 90, 20);
        assert!(text.contains("GATS"));
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn shows_a_placeholder_when_a_thumbnail_is_missing() {
        let mut form = SearchSelect::new("Pick a book", books())
            .with_thumbnail(|_| -> Option<Thumbnail> { None });

        let text = render_text(&mut form, 90, 20);

        assert!(text.contains("(no preview)"));
    }

    #[test]
    fn positions_the_terminal_cursor_after_the_query() {
        let mut form = SearchSelect::new("Pick a book", books());
        form.input.insert_str("abc");

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| form.render(frame)).unwrap();

        assert_eq!(terminal.backend().cursor_position(), Position::new(5, 1));
    }

    #[test]
    fn renders_in_small_terminals_without_panicking() {
        for (width, height) in [(20, 6), (39, 10), (40, 4), (10, 5)] {
            let mut form = SearchSelect::new("Pick a book", books());
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).unwrap();

            terminal.draw(|frame| form.render(frame)).unwrap();
        }
    }
}
