use super::{App, Screen};
use crate::store::{Result, Settings};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use std::path::{Path, PathBuf};

mod picking;

const LABEL_WIDTH: usize = 15;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Row {
    Folder,
    Jira,
    Archive,
}

impl Row {
    pub const ALL: [Self; 3] = [Self::Folder, Self::Jira, Self::Archive];

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
}

pub(super) struct SettingsView {
    pub config: Settings,
    pub selected: Row,
    pub rows_area: Rect,
    error: bool,
}

impl SettingsView {
    pub fn new(config: Settings) -> Self {
        Self {
            config,
            selected: Row::Folder,
            rows_area: Rect::default(),
            error: false,
        }
    }
}

pub(super) fn open(app: &mut App) -> Result<()> {
    app.settings = SettingsView::new(app.store.settings()?);
    app.screen = Screen::Settings;
    app.message.clear();
    Ok(())
}

pub(super) fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    let index = Row::ALL
        .iter()
        .position(|row| *row == app.settings.selected)
        .unwrap_or(0);
    match key.code {
        KeyCode::Esc => {
            if app.settings.config.sources.is_empty() {
                return Ok(true);
            }
            app.screen = Screen::List;
            app.message.clear();
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.settings.selected = Row::ALL[(index + 1).min(Row::ALL.len() - 1)]
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.settings.selected = Row::ALL[index.saturating_sub(1)]
        }
        KeyCode::Enter => activate(app),
        _ => {}
    }
    Ok(false)
}

pub(super) fn handle_mouse(app: &mut App, mouse: MouseEvent) -> Result<bool> {
    let area = app.settings.rows_area;
    if matches!(
        mouse.kind,
        MouseEventKind::Moved | MouseEventKind::Down(MouseButton::Left)
    ) && area.contains((mouse.column, mouse.row).into())
    {
        app.settings.selected = Row::ALL[usize::from(mouse.row - area.y)];
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            activate(app);
        }
    }
    Ok(false)
}

fn activate(app: &mut App) {
    let result = match app.settings.selected {
        Row::Folder => select_folder(app),
        Row::Jira => toggle_jira(app),
        Row::Archive => super::archive::open(app),
    };
    if let Err(error) = result {
        app.settings.error = true;
        app.message = error.to_string();
    }
}

fn toggle_jira(app: &mut App) -> Result<()> {
    let mut config = app.store.settings()?;
    config.jira = !config.jira;
    app.store.save_settings(&config)?;
    app.settings.config = config;
    app.settings.error = false;
    app.message.clear();
    Ok(())
}

fn select_folder(app: &mut App) -> Result<()> {
    let Some(source) = picking::choose(app)? else {
        return Ok(());
    };
    let mut config = app.store.settings()?;
    config.sources = vec![source];
    app.store.save_settings(&config)?;
    app.settings.config = config;
    app.settings.error = false;
    app.message.clear();
    let report = app.store.scan();
    app.refresh()?;
    let report = report?;
    app.scan_issues.clear();
    if !report.issues.is_empty() {
        super::scan::apply(app, report);
    }
    Ok(())
}

pub(super) fn path(value: &str) -> Result<PathBuf> {
    if value.is_empty() {
        return Err("Path cannot be empty".into());
    }
    if value == "~" || value.starts_with("~/") {
        let home = std::env::var_os("HOME").ok_or("HOME unavailable; enter an absolute path")?;
        return Ok(PathBuf::from(home).join(value.trim_start_matches('~').trim_start_matches('/')));
    }
    Ok(PathBuf::from(value))
}

/// Shortens the home directory to `~` for display.
pub(super) fn tilde(path: &Path) -> String {
    std::env::var_os("HOME")
        .and_then(|home| path.strip_prefix(home).ok())
        .map_or_else(
            || path.display().to_string(),
            |rest| format!("~/{}", rest.display()),
        )
}

/// Keeps the end of a path, its most specific part, when it cannot fit.
pub(super) fn fit_tail(value: &str, width: usize) -> String {
    let count = value.chars().count();
    if count <= width {
        return value.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let tail: String = value.chars().skip(count - (width - 1)).collect();
    format!("…{tail}")
}

fn value(app: &App, row: Row) -> String {
    match row {
        Row::Folder => app
            .settings
            .config
            .sources
            .first()
            .map_or_else(|| "No folder selected".into(), |source| tilde(&source.path)),
        Row::Jira if app.settings.config.jira => "on".into(),
        Row::Jira => "off".into(),
        Row::Archive => match app.archive.records.len() {
            0 => "empty".into(),
            1 => "1 spec".into(),
            count => format!("{count} specs"),
        },
    }
}

pub(super) fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let area = super::chrome::content(frame.area());
    app.settings.rows_area = Rect::default();
    if area.is_empty() {
        return;
    }
    // The last content row belongs to the shortcut line.
    let visible = Row::ALL
        .len()
        .min(usize::from(area.height.saturating_sub(1)));
    let muted = Style::default().fg(Color::Gray);
    let selection = Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    for (index, row) in Row::ALL.into_iter().take(visible).enumerate() {
        let label = format!("{:<LABEL_WIDTH$}", row.label());
        let value = fit_tail(
            &value(app, row),
            usize::from(area.width).saturating_sub(LABEL_WIDTH),
        );
        let line = Rect::new(area.x, area.y + index as u16, area.width, 1);
        if row == app.settings.selected {
            frame.render_widget(
                Paragraph::new(format!("{label}{value}")).style(selection),
                line,
            );
        } else {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(label, muted),
                    Span::raw(value),
                ])),
                line,
            );
        }
    }
    app.settings.rows_area = Rect::new(area.x, area.y, area.width, visible as u16);
    if app.settings.error && !app.message.is_empty() {
        let error_y = area.y + visible as u16 + 1;
        frame.render_widget(
            Paragraph::new(app.message.as_str())
                .style(Style::default().fg(Color::Red))
                .wrap(Wrap { trim: false }),
            Rect::new(
                area.x,
                error_y,
                area.width,
                area.bottom().saturating_sub(error_y + 2),
            ),
        );
    }
    super::chrome::draw_footer(
        frame,
        &format!(
            "j/k select · Enter {} · Esc back",
            app.settings.selected.verb()
        ),
    );
}
