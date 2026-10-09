//! Settings: one selectable row per setting, each showing its current value.

pub(crate) mod picker;

use crate::store::Result;
use crate::ui::notice::Tone;
use crate::ui::{App, Screen, chrome, text, theme};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

const LABEL_WIDTH: usize = 15;

/// A setting. Adding one means a variant here and an arm in each `match` below.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Row {
    Folder,
    Jira,
    Archive,
}

impl Row {
    const ALL: [Self; 3] = [Self::Folder, Self::Jira, Self::Archive];

    fn label(self) -> &'static str {
        match self {
            Self::Folder => "Specs folder",
            Self::Jira => "Jira",
            Self::Archive => "Archive",
        }
    }

    /// What Enter does on this row, as shown in the shortcut line.
    fn verb(self) -> &'static str {
        match self {
            Self::Folder => "change",
            Self::Jira => "toggle",
            Self::Archive => "open",
        }
    }

    fn value(self, app: &App) -> String {
        match self {
            Self::Folder => app.config.sources.first().map_or_else(
                || "No folder selected".into(),
                |source| text::tilde(&source.path),
            ),
            Self::Jira if app.config.jira => "on".into(),
            Self::Jira => "off".into(),
            Self::Archive => match app.archive.records.len() {
                0 => "empty".into(),
                1 => "1 spec".into(),
                count => format!("{count} specs"),
            },
        }
    }

    fn activate(self, app: &mut App) -> Result<()> {
        match self {
            Self::Folder => select_folder(app),
            Self::Jira => app.store.update_settings(|settings| {
                settings.jira = !settings.jira;
                Ok(())
            }),
            Self::Archive => super::archive::open(app),
        }
    }
}

pub(crate) struct View {
    pub selected: Row,
    pub rows_area: Rect,
}

impl Default for View {
    fn default() -> Self {
        Self {
            selected: Row::Folder,
            rows_area: Rect::default(),
        }
    }
}

pub(crate) fn open(app: &mut App) {
    app.settings = View::default();
    app.screen = Screen::Settings;
    app.notice.clear();
}

pub(crate) fn crumbs() -> Vec<String> {
    vec!["Settings".into()]
}

pub(crate) fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    let index = Row::ALL
        .iter()
        .position(|row| *row == app.settings.selected)
        .unwrap_or(0);
    match key.code {
        // With no folder there is no Inbox to go back to.
        KeyCode::Esc if app.config.sources.is_empty() => return Ok(true),
        KeyCode::Esc => {
            app.screen = Screen::List;
            app.notice.clear();
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.settings.selected = Row::ALL[(index + 1).min(Row::ALL.len() - 1)]
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.settings.selected = Row::ALL[index.saturating_sub(1)]
        }
        KeyCode::Enter => activate(app)?,
        _ => {}
    }
    Ok(false)
}

pub(crate) fn handle_mouse(app: &mut App, mouse: MouseEvent) -> Result<()> {
    let area = app.settings.rows_area;
    let clicked = mouse.kind == MouseEventKind::Down(MouseButton::Left);
    if (clicked || mouse.kind == MouseEventKind::Moved)
        && area.contains((mouse.column, mouse.row).into())
    {
        app.settings.selected = Row::ALL[usize::from(mouse.row - area.y)];
        if clicked {
            activate(app)?;
            app.refresh()?;
        }
    }
    Ok(())
}

fn activate(app: &mut App) -> Result<()> {
    app.notice.clear();
    app.settings.selected.activate(app)
}

/// Opens the native folder selector. Choosing a folder applies and scans it at once;
/// cancelling changes nothing.
fn select_folder(app: &mut App) -> Result<()> {
    let Some(source) = picker::choose_source(app)? else {
        return Ok(());
    };
    app.store.update_settings(|settings| {
        settings.sources = vec![source];
        Ok(())
    })?;
    let report = app.store.scan()?;
    super::scan::show(app, report);
    Ok(())
}

pub(crate) fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let area = chrome::content(frame.area());
    app.settings.rows_area = Rect::default();
    if area.is_empty() {
        return;
    }
    // The last content row belongs to the shortcut line.
    let visible = Row::ALL
        .len()
        .min(usize::from(area.height.saturating_sub(1)));
    for (index, row) in Row::ALL.into_iter().take(visible).enumerate() {
        let label = format!("{:<LABEL_WIDTH$}", row.label());
        let value = text::fit_tail(
            &row.value(app),
            usize::from(area.width).saturating_sub(LABEL_WIDTH),
        );
        let line = Rect::new(area.x, area.y + index as u16, area.width, 1);
        let paragraph = if row == app.settings.selected {
            Paragraph::new(format!("{label}{value}")).style(theme::selection())
        } else {
            Paragraph::new(Line::from(vec![
                Span::styled(label, theme::muted()),
                Span::raw(value),
            ]))
        };
        frame.render_widget(paragraph, line);
    }
    app.settings.rows_area = Rect::new(area.x, area.y, area.width, visible as u16);
    // A failed change explains itself below the rows, with room to wrap.
    if app.notice.tone() == Tone::Error && !app.notice.is_empty() {
        let y = area.y + visible as u16 + 1;
        frame.render_widget(
            Paragraph::new(app.notice.text())
                .style(theme::error())
                .wrap(Wrap { trim: false }),
            Rect::new(area.x, y, area.width, area.bottom().saturating_sub(y + 2)),
        );
    }
    let hints = format!(
        "j/k select · Enter {} · Esc back",
        app.settings.selected.verb()
    );
    chrome::draw_footer(frame, &hints);
}
