use super::{App, Screen, text, theme};
use crate::store::{Result, Settings};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

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
    app.store.update_settings(|settings| {
        settings.jira = !settings.jira;
        Ok(())
    })?;
    app.settings.config = app.store.settings()?;
    app.settings.error = false;
    app.message.clear();
    Ok(())
}

fn select_folder(app: &mut App) -> Result<()> {
    let Some(source) = picking::choose(app)? else {
        return Ok(());
    };
    app.store.update_settings(|settings| {
        settings.sources = vec![source];
        Ok(())
    })?;
    app.settings.config = app.store.settings()?;
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

fn value(app: &App, row: Row) -> String {
    match row {
        Row::Folder => app.settings.config.sources.first().map_or_else(
            || "No folder selected".into(),
            |source| text::tilde(&source.path),
        ),
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
    let muted = theme::muted();
    let selection = theme::selection();
    for (index, row) in Row::ALL.into_iter().take(visible).enumerate() {
        let label = format!("{:<LABEL_WIDTH$}", row.label());
        let value = text::fit_tail(
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
                .style(theme::error())
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
