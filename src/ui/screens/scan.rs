//! Scan results: shown when a scan found folders or files it could not read.

use crate::store::{Result, ScanReport};
use crate::ui::state::Scroll;
use crate::ui::{App, Screen, chrome, theme};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::{Paragraph, Wrap};

#[derive(Default)]
pub(crate) struct View {
    summary: String,
    issues: Vec<String>,
    scroll: Scroll,
}

impl View {
    pub(crate) fn new(report: ScanReport) -> Self {
        Self {
            summary: report.summary(),
            issues: report.issues,
            scroll: Scroll::default(),
        }
    }
}

/// Records a scan's outcome and opens this screen when it found problems.
pub(crate) fn show(app: &mut App, report: ScanReport) {
    app.scan = View::new(report);
    if !app.scan.issues.is_empty() {
        app.screen = Screen::Scan;
    }
}

pub(crate) fn crumbs() -> Vec<String> {
    vec!["Settings".into(), "Scan results".into()]
}

pub(crate) fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => app.screen = Screen::List,
        KeyCode::Char('s') => super::settings::open(app),
        KeyCode::Down | KeyCode::Char('j') => app.scan.scroll.down(1),
        KeyCode::Up | KeyCode::Char('k') => app.scan.scroll.up(1),
        _ => {}
    }
    Ok(false)
}

pub(crate) fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let area = chrome::content(frame.area());
    let blocks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(2),
        Constraint::Length(2),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(app.scan.summary.as_str()).wrap(Wrap { trim: false }),
        blocks[0],
    );
    let text = format!(
        "Correct the folder or permissions, then reopen the Inbox to scan again.\n\n{}",
        app.scan.issues.join("\n\n")
    );
    let content = Paragraph::new(text)
        .style(theme::warning())
        .wrap(Wrap { trim: false });
    let overflow = content
        .line_count(blocks[1].width)
        .saturating_sub(usize::from(blocks[1].height));
    app.scan.scroll.limit(overflow);
    frame.render_widget(content.scroll((app.scan.scroll.offset(), 0)), blocks[1]);
    chrome::draw_footer(frame, "j/k scroll · s settings · Esc back");
}
