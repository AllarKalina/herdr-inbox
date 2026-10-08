use super::{App, Screen};
use crate::store::{Result, ScanReport};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Margin};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Paragraph, Wrap};

pub(super) fn apply(app: &mut App, report: ScanReport) {
    app.message = report.summary();
    app.scan_issues = report.issues;
    if !app.scan_issues.is_empty() {
        app.screen = Screen::ScanResult;
        app.reader_scroll = 0;
    }
}

pub(super) fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => app.screen = Screen::List,
        KeyCode::Char('s') => super::settings::open(app)?,
        KeyCode::Down | KeyCode::Char('j') => {
            app.reader_scroll = app
                .reader_scroll
                .saturating_add(1)
                .min(app.reader_max_scroll)
        }
        KeyCode::Up | KeyCode::Char('k') => app.reader_scroll = app.reader_scroll.saturating_sub(1),
        _ => {}
    }
    Ok(false)
}

pub(super) fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let area = frame.area().inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let blocks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(2),
        Constraint::Length(2),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new("Inbox / Scan results").style(
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        ),
        blocks[0],
    );
    frame.render_widget(
        Paragraph::new(app.message.as_str()).wrap(Wrap { trim: false }),
        blocks[1],
    );
    let text = format!(
        "Existing items stay in your inbox. Correct the paths or permissions, then rescan.\n\n{}",
        app.scan_issues.join("\n\n")
    );
    let content = Paragraph::new(text)
        .style(Style::default().fg(Color::Yellow))
        .wrap(Wrap { trim: false });
    app.reader_max_scroll = content
        .line_count(blocks[2].width)
        .saturating_sub(blocks[2].height as usize)
        .min(u16::MAX as usize) as u16;
    frame.render_widget(content.scroll((app.reader_scroll, 0)), blocks[2]);
    frame.render_widget(
        Paragraph::new("j/k scroll · s settings\nEnter / Esc back to Inbox")
            .style(Style::default().fg(Color::Gray)),
        blocks[3],
    );
}
