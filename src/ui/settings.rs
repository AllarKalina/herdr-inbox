use super::{App, Screen};
use crate::store::{Result, Settings};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Paragraph, Wrap};
use std::path::PathBuf;

mod picking;

pub(super) struct SettingsView {
    pub config: Settings,
    pub change_area: Rect,
    error: bool,
}

impl SettingsView {
    pub fn new(config: Settings) -> Self {
        Self {
            config,
            change_area: Rect::default(),
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
    match key.code {
        KeyCode::Esc => {
            if app.settings.config.sources.is_empty() {
                return Ok(true);
            }
            app.screen = Screen::List;
            app.message.clear();
        }
        KeyCode::Enter => change(app),
        _ => {}
    }
    Ok(false)
}

pub(super) fn handle_mouse(app: &mut App, mouse: MouseEvent) -> Result<bool> {
    if mouse.kind == MouseEventKind::Down(MouseButton::Left)
        && app
            .settings
            .change_area
            .contains((mouse.column, mouse.row).into())
    {
        change(app);
    }
    Ok(false)
}

fn change(app: &mut App) {
    if let Err(error) = select_folder(app) {
        app.settings.error = true;
        app.message = error.to_string();
    }
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

pub(super) fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let area = super::chrome::content(frame.area());
    app.settings.change_area = Rect::default();
    if area.is_empty() {
        return;
    }
    frame.render_widget(
        Paragraph::new("Specs folder").style(Style::default().fg(Color::Gray)),
        Rect::new(area.x, area.y, area.width, 1),
    );
    let path = app
        .settings
        .config
        .sources
        .first()
        .map(|source| source.path.display().to_string())
        .unwrap_or_else(|| "No folder selected".into());
    let path = Paragraph::new(path).wrap(Wrap { trim: false });
    let error_height = if app.settings.error {
        Paragraph::new(app.message.as_str())
            .wrap(Wrap { trim: false })
            .line_count(area.width)
            .min(area.height.saturating_sub(6) as usize) as u16
    } else {
        0
    };
    let path_height = path
        .line_count(area.width)
        .min(area.height.saturating_sub(5 + error_height) as usize)
        .max(1) as u16;
    frame.render_widget(path, Rect::new(area.x, area.y + 1, area.width, path_height));
    let change_y = area.y + path_height + 2;
    if change_y < area.bottom().saturating_sub(1) {
        app.settings.change_area = Rect::new(area.x, change_y, 8.min(area.width), 1);
        frame.render_widget(
            Paragraph::new("› Change").style(
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ),
            app.settings.change_area,
        );
    }
    if app.settings.error && !app.message.is_empty() {
        let error_y = change_y + 2;
        frame.render_widget(
            Paragraph::new(app.message.as_str())
                .style(Style::default().fg(Color::Red))
                .wrap(Wrap { trim: false }),
            Rect::new(
                area.x,
                error_y,
                area.width,
                area.bottom().saturating_sub(error_y + 1),
            ),
        );
    }
    frame.render_widget(
        Paragraph::new("Enter change · Esc back").style(Style::default().fg(Color::Gray)),
        Rect::new(area.x, area.bottom() - 1, area.width, 1),
    );
}
