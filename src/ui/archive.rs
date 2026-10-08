use super::settings::{fit_tail, tilde};
use super::{App, Screen, detail::fit_label, tree};
use crate::store::{Record, Result};
use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap};

#[derive(Default)]
pub(super) struct ArchiveView {
    pub records: Vec<Record>,
    pub selected: usize,
    offset: usize,
    confirm_delete: bool,
    list_area: Rect,
}

impl ArchiveView {
    fn current(&self) -> Option<&Record> {
        self.records.get(self.selected)
    }
}

/// Rereads the archive from disk, keeping the selection on the same spec when it is still there.
pub(super) fn reload(app: &mut App) -> Result<()> {
    let id = app.archive.current().map(|record| record.id.clone());
    let mut records = app.store.trash_list()?;
    records.sort_by(|a, b| {
        a.display_title()
            .to_lowercase()
            .cmp(&b.display_title().to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    let kept = id
        .as_ref()
        .and_then(|id| records.iter().position(|record| &record.id == id));
    if kept.is_none() {
        app.archive.confirm_delete = false;
    }
    app.archive.selected =
        kept.unwrap_or(app.archive.selected.min(records.len().saturating_sub(1)));
    app.archive.records = records;
    Ok(())
}

pub(super) fn open(app: &mut App) -> Result<()> {
    app.archive.selected = 0;
    app.archive.confirm_delete = false;
    app.message.clear();
    app.screen = Screen::Archive;
    reload(app)
}

pub(super) fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    if app.archive.confirm_delete {
        match key.code {
            KeyCode::Esc => app.archive.confirm_delete = false,
            KeyCode::Enter => {
                app.archive.confirm_delete = false;
                delete(app)?;
            }
            _ => {}
        }
        return Ok(false);
    }
    let last = app.archive.records.len().saturating_sub(1);
    match key.code {
        KeyCode::Esc => {
            app.screen = Screen::Settings;
            app.message.clear();
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.archive.selected = (app.archive.selected + 1).min(last)
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.archive.selected = app.archive.selected.saturating_sub(1)
        }
        KeyCode::Char('r') => restore(app)?,
        KeyCode::Char('d') if app.archive.current().is_some() => {
            app.archive.confirm_delete = true;
            app.message.clear();
        }
        _ => {}
    }
    Ok(false)
}

pub(super) fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    let area = app.archive.list_area;
    if app.archive.confirm_delete || !area.contains((mouse.column, mouse.row).into()) {
        return;
    }
    let last = app.archive.records.len().saturating_sub(1);
    match mouse.kind {
        MouseEventKind::Moved | MouseEventKind::Down(_) => {
            let index = app.archive.offset + usize::from(mouse.row - area.y);
            if index <= last {
                app.archive.selected = index;
            }
        }
        MouseEventKind::ScrollDown => app.archive.selected = (app.archive.selected + 1).min(last),
        MouseEventKind::ScrollUp => app.archive.selected = app.archive.selected.saturating_sub(1),
        _ => {}
    }
}

fn restore(app: &mut App) -> Result<()> {
    let Some(id) = app.archive.current().map(|record| record.id.clone()) else {
        return Ok(());
    };
    let record = app.store.restore(&id)?;
    app.refresh()?;
    let title = record.display_title();
    app.message = if app.records.iter().any(|shown| shown.id == record.id) {
        format!("Restored {title}")
    } else {
        format!("Restored {title}; hidden until its file is inside the specs folder")
    };
    Ok(())
}

fn delete(app: &mut App) -> Result<()> {
    let Some(record) = app.archive.current().cloned() else {
        return Ok(());
    };
    let (_, moved) = app.store.delete_archived(&record.id)?;
    app.refresh()?;
    let title = record.display_title();
    app.message = if moved {
        format!("Deleted {title}; file moved to the macOS Trash")
    } else {
        format!("Deleted {title}")
    };
    Ok(())
}

/// Where the spec lives: relative to the specs folder when inside it, otherwise in full.
fn location(app: &App, record: &Record) -> String {
    let sources = &app.settings.config.sources;
    match &record.source_relative_path {
        Some(relative) if tree::includes(record, sources) => relative.display().to_string(),
        _ => tilde(&record.spec_path),
    }
}

pub(super) fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let content = super::chrome::content(frame.area());
    let footer = super::chrome::footer_area(frame.area());
    let confirming = app.archive.confirm_delete;
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(if confirming { 8 } else { 0 }),
            Constraint::Length(if app.message.is_empty() { 1 } else { 2 }),
        ])
        .split(content);
    app.archive.list_area = Rect::default();
    if app.archive.records.is_empty() {
        frame.render_widget(
            Paragraph::new("No archived specs.").style(Style::default().fg(Color::Gray)),
            areas[0],
        );
    } else {
        draw_list(frame, app, areas[0]);
    }
    if confirming && let Some(record) = app.archive.current() {
        let width = usize::from(areas[1].width.saturating_sub(2));
        let file = if record.spec_path.symlink_metadata().is_ok() {
            "The file moves to the macOS Trash."
        } else {
            "The file is already gone."
        };
        let text = format!(
            "DELETE SPEC\n{}\n{}\n{file}\nIts archived record is deleted for good.",
            fit_label(record.display_title(), width),
            fit_tail(&tilde(&record.spec_path), width),
        );
        frame.render_widget(
            Paragraph::new(text).wrap(Wrap { trim: false }).block(
                Block::default()
                    .title(" Confirm delete ")
                    .borders(Borders::ALL),
            ),
            areas[1],
        );
    }
    if !app.message.is_empty() {
        frame.render_widget(
            Paragraph::new(fit_label(&app.message, usize::from(footer.width))),
            Rect::new(footer.x, footer.y.saturating_sub(1), footer.width, 1),
        );
    }
    let hints = if confirming {
        "Enter delete this spec · Esc cancel"
    } else if app.archive.records.is_empty() {
        "Esc back"
    } else if footer.width >= 42 {
        "j/k select · r restore · d delete · Esc back"
    } else {
        "r restore · d delete · Esc back"
    };
    super::chrome::draw_footer(frame, hints);
}

fn draw_list(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let locations: Vec<String> = app
        .archive
        .records
        .iter()
        .map(|record| location(app, record))
        .collect();
    // Narrow popups keep the whole row for the title.
    let location_width = if frame.area().width < 64 {
        0
    } else {
        locations
            .iter()
            .map(|location| location.chars().count())
            .max()
            .unwrap_or(0)
            .min(usize::from(area.width / 2))
    };
    let title_width = usize::from(area.width).saturating_sub(location_width + 2);
    let rows = app
        .archive
        .records
        .iter()
        .zip(&locations)
        .enumerate()
        .map(|(index, (record, location))| {
            let title = if record.spec_path.is_file() {
                record.display_title().to_owned()
            } else {
                format!("{} [unavailable]", record.display_title())
            };
            let location = Cell::from(fit_tail(location, location_width)).style(
                if index == app.archive.selected {
                    Style::default()
                } else {
                    Style::default().fg(Color::Gray)
                },
            );
            Row::new(vec![Cell::from(fit_label(&title, title_width)), location])
        })
        .collect::<Vec<_>>();
    let table = Table::new(
        rows,
        [
            Constraint::Fill(1),
            Constraint::Length(location_width as u16),
        ],
    )
    .column_spacing(2)
    .row_highlight_style(
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    let mut state = TableState::default()
        .with_offset(app.archive.offset)
        .with_selected(Some(app.archive.selected));
    frame.render_stateful_widget(table, area, &mut state);
    app.archive.offset = state.offset();
    app.archive.list_area = area;
}
