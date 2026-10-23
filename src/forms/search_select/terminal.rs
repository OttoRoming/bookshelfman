//! Inline terminal lifecycle for the form.
//!
//! The session deliberately initializes ratatui manually instead of using
//! `ratatui::init`: forms must render from the current cursor row and must
//! never enter the alternate screen. Ratatui's [`Viewport::Inline`] handles
//! reserving rows and scrolling, while this module owns raw mode and cleanup.

use std::io::{self, IsTerminal, Stdout, stdin, stdout};

use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::terminal;
use ratatui::layout::Rect;
use ratatui::{Frame, Terminal, TerminalOptions, Viewport};

use super::Error;

/// The form needs at least this many columns.
pub(crate) const MIN_WIDTH: u16 = 20;
/// The form needs at least this many rows.
pub(crate) const MIN_HEIGHT: u16 = 6;

/// A terminal with a form-sized inline viewport.
pub(crate) struct Session {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    last_area: Option<Rect>,
    _raw_mode: RawModeGuard,
}

impl Session {
    /// Creates an inline session that is `form_height` rows tall.
    ///
    /// The viewport is anchored to the cursor's current row and may scroll the
    /// terminal to fit. It does not use the alternate screen.
    pub(crate) fn start(form_height: u16) -> Result<Self, Error> {
        if !stdin().is_terminal() || !stdout().is_terminal() {
            return Err(Error::NotATerminal);
        }

        let (width, height) = terminal::size()?;

        if width < MIN_WIDTH || height < MIN_HEIGHT {
            return Err(Error::TerminalTooSmall {
                width,
                height,
                min_width: MIN_WIDTH,
                min_height: MIN_HEIGHT,
            });
        }

        let raw_mode = RawModeGuard::enable()?;
        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(form_height),
            },
        )?;

        terminal.hide_cursor()?;
        // Inline viewports reserve blank rows by appending lines. Clearing
        // once makes sure the form cannot show stale output underneath.
        terminal.clear()?;

        Ok(Self {
            terminal,
            last_area: None,
            _raw_mode: raw_mode,
        })
    }

    /// Draws a frame and remembers its area for cleanup.
    pub(crate) fn draw(&mut self, render: impl FnOnce(&mut Frame)) -> Result<(), Error> {
        let terminal = &mut self.terminal;
        let last_area = &mut self.last_area;

        terminal.draw(|frame| {
            // `CompletedFrame::area` is the whole terminal, but for an inline
            // viewport cleanup must return to the viewport's own origin. Store
            // it before rendering so even a panicking render can clean up.
            *last_area = Some(frame.area());
            render(frame);
        })?;

        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let origin = self.last_area.map_or(0, |area| area.y);

        // The form disappears when it is done; the caller can print the
        // selected value where the form used to be.
        let _ = self.terminal.clear();
        let _ = self.terminal.show_cursor();
        let _ = self.terminal.set_cursor_position((0, origin));
    }
}

/// Restores raw mode even if the terminal is dropped after a panic.
struct RawModeGuard;

impl RawModeGuard {
    fn enable() -> io::Result<Self> {
        terminal::enable_raw_mode()?;

        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}
