//! The full spec, scrollable.

use crate::store::{Record, Result};
use crate::ui::scroll::Scroll;
use crate::ui::{App, Screen, chrome};
use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::{Paragraph, Wrap};
use std::fs;

#[derive(Default)]
pub(crate) struct View {
    scroll: Scroll,
}

/// Opens the reader on the item shown in the detail view.
pub(crate) fn open(app: &mut App) {
    app.detail.feedback = None;
    app.reader.scroll = Scroll::default();
    app.screen = Screen::Reader;
}

pub(crate) fn crumbs(app: &App) -> Vec<String> {
    let mut crumbs = super::detail::crumbs(app);
    crumbs.push("FULL SPEC".into());
    crumbs
}

/// The spec's text, or what stands in for it when the file cannot be shown.
pub(crate) fn spec_text(record: &Record) -> String {
    match fs::read_to_string(&record.spec_path) {
        Ok(text) if text.trim().is_empty() => "Spec file is empty.".into(),
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            "Spec unavailable. Restore the file at its original path.".into()
        }
        Err(error) => format!("Cannot read spec: {error}"),
    }
}

pub(crate) fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    let scroll = &mut app.reader.scroll;
    match key.code {
        KeyCode::Esc | KeyCode::Char('r') => app.screen = Screen::Detail,
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Char('j') | KeyCode::Down => scroll.down(1),
        KeyCode::Char('k') | KeyCode::Up => scroll.up(1),
        KeyCode::Char('J') => scroll.down(10),
        KeyCode::Char('K') => scroll.up(10),
        KeyCode::PageDown => scroll.down(15),
        KeyCode::PageUp => scroll.up(15),
        KeyCode::Char('g') => scroll.top(),
        _ => {}
    }
    Ok(false)
}

pub(crate) fn handle_mouse(app: &mut App, mouse: MouseEvent) -> Result<()> {
    match mouse.kind {
        MouseEventKind::ScrollDown => app.reader.scroll.down(3),
        MouseEventKind::ScrollUp => app.reader.scroll.up(3),
        _ => {}
    }
    Ok(())
}

pub(crate) fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let Some(record) = app.current() else {
        return;
    };
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(2)])
        .split(chrome::content(frame.area()));
    let paragraph = Paragraph::new(spec_text(record)).wrap(Wrap { trim: false });
    let content_lines = paragraph.line_count(areas[0].width);
    let viewport_lines = usize::from(areas[0].height);
    // Scrolling stops two rows after the last line, so the end of the spec is unmistakable.
    app.reader.scroll.limit(if content_lines > viewport_lines {
        content_lines + 2 - viewport_lines
    } else {
        0
    });
    frame.render_widget(paragraph.scroll((app.reader.scroll.offset(), 0)), areas[0]);
    chrome::draw_footer(frame, "j/k scroll · Shift+J/K 10 lines");
}
