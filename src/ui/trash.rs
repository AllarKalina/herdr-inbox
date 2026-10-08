use super::{App, Screen};
use crate::store::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Margin};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Wrap};

pub(super) fn open(app: &mut App) -> Result<()> {
    app.trash = app.store.trash_list()?;
    app.trash_selected = 0;
    app.trash_return = app.screen;
    app.screen = Screen::Trash;
    app.message.clear();
    Ok(())
}

pub(super) fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Esc => app.screen = app.trash_return,
        KeyCode::Down | KeyCode::Char('j') => {
            app.trash_selected = (app.trash_selected + 1).min(app.trash.len().saturating_sub(1))
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.trash_selected = app.trash_selected.saturating_sub(1)
        }
        KeyCode::Enter => {
            if let Some(record) = app.trash.get(app.trash_selected) {
                let restored = app.store.restore(&record.id)?;
                app.message = format!("Restored {}", restored.display_title());
                app.screen = app.trash_return;
                app.refresh()?;
                app.selected = app
                    .records
                    .iter()
                    .position(|record| record.id == restored.id)
                    .unwrap_or(0);
            }
        }
        _ => {}
    }
    Ok(false)
}

pub(super) fn draw(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area().inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let blocks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(2),
        Constraint::Length(5),
        Constraint::Length(2),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new("Inbox / Archived items").style(
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        ),
        blocks[0],
    );
    let offset = app
        .trash_selected
        .saturating_add(1)
        .saturating_sub(blocks[1].height as usize);
    let lines = if app.trash.is_empty() {
        vec![
            Line::from("No archived items. Esc goes back.").style(Style::default().fg(Color::Gray)),
        ]
    } else {
        app.trash
            .iter()
            .enumerate()
            .skip(offset)
            .map(|(index, record)| {
                Line::from(format!(
                    "{}{}",
                    if index == app.trash_selected {
                        "› "
                    } else {
                        "  "
                    },
                    record.display_title()
                ))
                .style(if index == app.trash_selected {
                    Style::default()
                        .fg(Color::LightCyan)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                } else {
                    Style::default()
                })
            })
            .collect()
    };
    frame.render_widget(Paragraph::new(lines), blocks[1]);
    if let Some(record) = app.trash.get(app.trash_selected) {
        frame.render_widget(
            Paragraph::new(format!(
                "{}\n{}\nRestores the same item and workflow progress.",
                record.id,
                record.spec_path.display()
            ))
            .wrap(Wrap { trim: false }),
            blocks[2],
        );
    }
    frame.render_widget(
        Paragraph::new("j/k choose · Enter restore\nEsc back")
            .style(Style::default().fg(Color::Gray)),
        blocks[3],
    );
}
