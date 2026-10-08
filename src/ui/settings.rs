use super::{App, Screen};
use crate::store::{Result, Settings, SpecSource};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Margin};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use std::path::PathBuf;

mod picking;

#[derive(Clone)]
enum Field {
    AddSource,
    AddContext,
    Include(usize),
    Exclude(usize),
    Context(usize),
    Client,
    Workspace,
    Relocate(usize),
    ConfirmRelocate(usize, PathBuf),
}

pub(super) struct SettingsView {
    pub draft: Settings,
    selected: usize,
    edit: Option<Field>,
    input: String,
    pub first_use: bool,
    error: bool,
    horizontal_scroll: usize,
}

impl SettingsView {
    pub fn new(draft: Settings, first_use: bool) -> Self {
        Self {
            draft,
            selected: 0,
            edit: None,
            input: String::new(),
            first_use,
            error: false,
            horizontal_scroll: 0,
        }
    }

    fn rows(&self) -> Vec<String> {
        let mut rows = Vec::new();
        for source in &self.draft.sources {
            rows.push(format!("Specs: {}", source.path.display()));
            rows.push(format!(
                "  Recursive: {}",
                if source.recursive { "yes" } else { "no" }
            ));
            rows.push(format!("  Include: {}", source.include.join(", ")));
            rows.push(format!("  Exclude: {}", source.exclude.join(", ")));
        }
        for path in &self.draft.context_paths {
            rows.push(format!("Context: {}", path.display()));
        }
        rows.push("Add specs folder".into());
        rows.push("Add context file or folder".into());
        rows.push(format!(
            "Client: {}",
            self.draft
                .preferred_client
                .as_deref()
                .unwrap_or("choose each time")
        ));
        rows.push(format!("Workspace: {}", self.draft.workspace));
        rows.push("Save and scan".into());
        rows.push("Restore archived item".into());
        rows
    }

    fn begin(&mut self, field: Field, input: String) {
        self.edit = Some(field);
        self.input = input;
        self.error = false;
    }

    fn label(&self) -> &str {
        match self.edit {
            Some(Field::AddSource) => "Specs folder (existing directory)",
            Some(Field::AddContext | Field::Context(_)) => "Context path (existing file or folder)",
            Some(Field::Include(_)) => "Include globs (comma-separated)",
            Some(Field::Exclude(_)) => "Exclude globs (comma-separated; blank = none)",
            Some(Field::Client) => "Client: codex / opus (blank = choose)",
            Some(Field::Workspace) => "Herdr workspace",
            Some(Field::Relocate(_)) => "New location of SAME specs folder",
            Some(Field::ConfirmRelocate(_, _)) => "Same source? Enter confirms; Esc cancels",
            None => "",
        }
    }
}

pub(super) fn open(app: &mut App) -> Result<()> {
    app.settings = SettingsView::new(app.store.settings()?, false);
    app.screen = Screen::Settings;
    app.message.clear();
    Ok(())
}

pub(super) fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    // Keep invalid entries visible so users can correct them rather than start over.
    let result = handle(app, key);
    if let Err(error) = result {
        app.settings.error = true;
        app.message = error.to_string();
    }
    Ok(false)
}

fn handle(app: &mut App, key: KeyEvent) -> Result<()> {
    if let Some(field) = app.settings.edit.clone() {
        match key.code {
            KeyCode::Esc => {
                app.settings.edit = None;
                app.settings.input.clear();
                app.message.clear();
            }
            KeyCode::Backspace => {
                app.settings.input.pop();
            }
            KeyCode::Char(ch) if !matches!(field, Field::ConfirmRelocate(..)) => {
                app.settings.input.push(ch)
            }
            KeyCode::Enter => submit_field(app, field)?,
            _ => {}
        }
        return Ok(());
    }
    let rows = app.settings.rows().len();
    match key.code {
        KeyCode::Right => {
            app.settings.horizontal_scroll = (app.settings.horizontal_scroll + 4).min(
                app.settings.rows()[app.settings.selected]
                    .chars()
                    .count()
                    .saturating_sub(1),
            )
        }
        KeyCode::Left => {
            app.settings.horizontal_scroll = app.settings.horizontal_scroll.saturating_sub(4)
        }
        KeyCode::Esc => {
            app.screen = Screen::List;
            app.message = "Settings not saved".into();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.settings.selected = (app.settings.selected + 1).min(rows - 1);
            app.settings.horizontal_scroll = 0;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.settings.selected = app.settings.selected.saturating_sub(1);
            app.settings.horizontal_scroll = 0;
        }
        KeyCode::Char('a') => picking::choose(app, Field::AddSource)?,
        KeyCode::Char('c') => picking::choose(app, Field::AddContext)?,
        KeyCode::Char('p') => picking::type_path(app),
        KeyCode::Char('d') => remove_selected(app),
        KeyCode::Char('m') if app.settings.selected < app.settings.draft.sources.len() * 4 => {
            let index = app.settings.selected / 4;
            picking::choose(app, Field::Relocate(index))?;
        }
        KeyCode::Char('s') => save(app)?,
        KeyCode::Enter => edit_selected(app)?,
        _ => {}
    }
    Ok(())
}

fn remove_selected(app: &mut App) {
    let selected = app.settings.selected;
    let source_rows = app.settings.draft.sources.len() * 4;
    if selected < source_rows {
        app.settings.draft.sources.remove(selected / 4);
        app.message =
            "Source removed from settings; files and inbox items stay in place. Save to apply."
                .into();
    } else if selected < source_rows + app.settings.draft.context_paths.len() {
        app.settings
            .draft
            .context_paths
            .remove(selected - source_rows);
        app.message = "Context reference removed. Save to apply.".into();
    }
    app.settings.selected = app.settings.selected.min(app.settings.rows().len() - 1);
}

fn edit_selected(app: &mut App) -> Result<()> {
    let selected = app.settings.selected;
    let sources = app.settings.draft.sources.len() * 4;
    let contexts = app.settings.draft.context_paths.len();
    if selected < sources {
        let index = selected / 4;
        let source = &mut app.settings.draft.sources[index];
        match selected % 4 {
            0 => picking::choose(app, Field::Relocate(index))?,
            1 => source.recursive = !source.recursive,
            2 => {
                let input = source.include.join(", ");
                app.settings.begin(Field::Include(index), input);
            }
            _ => {
                let input = source.exclude.join(", ");
                app.settings.begin(Field::Exclude(index), input);
            }
        }
    } else if selected < sources + contexts {
        let index = selected - sources;
        picking::choose(app, Field::Context(index))?;
    } else {
        match selected - sources - contexts {
            0 => picking::choose(app, Field::AddSource)?,
            1 => picking::choose(app, Field::AddContext)?,
            2 => {
                let input = app
                    .settings
                    .draft
                    .preferred_client
                    .clone()
                    .unwrap_or_default();
                app.settings.begin(Field::Client, input);
            }
            3 => {
                let input = app.settings.draft.workspace.clone();
                app.settings.begin(Field::Workspace, input);
            }
            4 => save(app)?,
            _ => super::trash::open(app)?,
        }
    }
    Ok(())
}

fn submit_field(app: &mut App, field: Field) -> Result<()> {
    let value = app.settings.input.trim().to_string();
    match field {
        Field::AddSource => {
            app.settings
                .draft
                .sources
                .push(SpecSource::new(path(&value)?)?);
        }
        Field::AddContext | Field::Context(_) => {
            let target = path(&value)?;
            if !target.exists() {
                return Err(format!(
                    "Context unavailable: {}. Choose an existing path.",
                    target.display()
                )
                .into());
            }
            if let Field::Context(index) = field {
                app.settings.draft.context_paths[index] = target;
            } else {
                app.settings.draft.context_paths.push(target);
            }
        }
        Field::Include(index) => app.settings.draft.sources[index].include = patterns(&value),
        Field::Exclude(index) => app.settings.draft.sources[index].exclude = patterns(&value),
        Field::Client => {
            if !value.is_empty() {
                crate::launch::Profile::parse(&value)?;
            }
            app.settings.draft.preferred_client = if value.is_empty() { None } else { Some(value) };
        }
        Field::Workspace => {
            if value.is_empty() {
                return Err("Workspace cannot be empty".into());
            }
            app.settings.draft.workspace = value;
        }
        Field::Relocate(index) => {
            let target = SpecSource::new(path(&value)?)?.path;
            app.settings
                .begin(Field::ConfirmRelocate(index, target), String::new());
            return Ok(());
        }
        Field::ConfirmRelocate(index, target) => {
            let id = app.settings.draft.sources[index].id.clone();
            if !app
                .store
                .settings()?
                .sources
                .iter()
                .any(|source| source.id == id)
            {
                return Err("Save this new source first, then relocate it".into());
            }
            app.message = app.store.relocate_source(&id, target.clone())?.summary();
            app.settings.draft.sources[index].path = target;
            app.refresh()?;
        }
    }
    app.settings.edit = None;
    app.settings.input.clear();
    app.settings.error = false;
    Ok(())
}

fn patterns(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
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

fn save(app: &mut App) -> Result<()> {
    app.store.save_settings(&app.settings.draft)?;
    let report = app.store.scan()?;
    app.settings.first_use = false;
    app.screen = Screen::List;
    super::scan::apply(app, report);
    app.refresh()?;
    Ok(())
}

pub(super) fn draw(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area().inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let blocks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(if app.settings.edit.is_some() { 4 } else { 0 }),
        Constraint::Length(4),
    ])
    .split(area);
    let title = if app.settings.first_use {
        "Inbox / Connect your specs"
    } else {
        "Inbox / Settings"
    };
    frame.render_widget(
        Paragraph::new(title).style(
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        ),
        blocks[0],
    );
    frame.render_widget(
        Paragraph::new("Your folders stay in place. Metadata stays on this Mac.")
            .style(Style::default().fg(Color::Gray))
            .wrap(Wrap { trim: false }),
        blocks[1],
    );
    let rows = app.settings.rows();
    let offset = app
        .settings
        .selected
        .saturating_add(1)
        .saturating_sub(blocks[2].height as usize);
    let lines = rows
        .iter()
        .enumerate()
        .skip(offset)
        .map(|(index, row)| {
            Line::from(vec![
                Span::styled(
                    if index == app.settings.selected {
                        "› "
                    } else {
                        "  "
                    },
                    Style::default().fg(Color::LightCyan),
                ),
                Span::styled(
                    if index == app.settings.selected {
                        row.chars()
                            .skip(app.settings.horizontal_scroll)
                            .collect::<String>()
                    } else {
                        row.clone()
                    },
                    if index == app.settings.selected {
                        Style::default()
                            .fg(Color::LightCyan)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                    } else {
                        Style::default()
                    },
                ),
            ])
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), blocks[2]);
    if app.settings.edit.is_some() {
        let mut lines =
            vec![Line::from(app.settings.label()).style(Style::default().fg(Color::LightBlue))];
        let width = blocks[3].width.saturating_sub(1) as usize;
        let tail = app
            .settings
            .input
            .chars()
            .rev()
            .take(width)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>();
        if let Some(Field::ConfirmRelocate(index, target)) = &app.settings.edit {
            lines.push(Line::from(format!(
                "{} → {}",
                app.settings.draft.sources[*index].path.display(),
                target.display()
            )));
        } else {
            lines.push(Line::from(format!("{tail}█")));
        }
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), blocks[3]);
    }
    let help = if app.settings.edit.is_some() {
        "Enter accept · Esc cancel"
    } else {
        "j/k move · Enter choose · s save\np path · a specs · c ctx · d rm"
    };
    let message = if app.message.is_empty()
        && rows[app.settings.selected].chars().count() > blocks[2].width as usize
    {
        "Left/Right scroll long paths"
    } else {
        app.message.as_str()
    };
    let footer = Layout::vertical([Constraint::Length(2), Constraint::Length(2)]).split(blocks[4]);
    frame.render_widget(
        Paragraph::new(message)
            .style(Style::default().fg(if app.settings.error {
                Color::Red
            } else {
                Color::Gray
            }))
            .wrap(Wrap { trim: false }),
        footer[0],
    );
    frame.render_widget(
        Paragraph::new(help).style(Style::default().fg(Color::Gray)),
        footer[1],
    );
}
