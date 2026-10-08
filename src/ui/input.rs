use super::{App, ChoicePurpose, DetailAction, Milestone, Prompt, Screen};
use crate::launch;
use crate::store::{Change, Result};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use std::io::stdout;
use std::process::Command;

pub(super) fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    if key.kind != KeyEventKind::Press {
        return Ok(false);
    }
    if app.screen == Screen::ScanResult {
        return super::scan::handle_key(app, key);
    }
    if app.screen == Screen::Trash {
        return super::trash::handle_key(app, key);
    }
    if app.screen == Screen::Settings {
        return super::settings::handle_key(app, key);
    }
    if let Some(selected) = app.choice_selected {
        match key.code {
            KeyCode::Esc => app.choice_selected = None,
            KeyCode::Char('j') | KeyCode::Down => {
                app.choice_selected = Some((selected + 1).min(app.choices.len() - 1))
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.choice_selected = Some(selected.saturating_sub(1))
            }
            KeyCode::Enter => {
                let profile = app.choices[selected];
                match app.choice_purpose.clone() {
                    ChoicePurpose::NewSpec => {
                        app.choice_selected = None;
                        app.begin(Prompt::LaunchWorkspace { profile });
                        app.input = app.store.settings()?.workspace;
                    }
                    ChoicePurpose::Refine { id } => {
                        let mut options = launch::Options::for_profile(profile);
                        options.workspace = app.store.settings()?.workspace;
                        launch::refine(&app.store, &id, options)?;
                        app.choice_selected = None;
                        app.should_exit = true;
                    }
                }
            }
            _ => {}
        }
        return Ok(app.should_exit);
    }
    if matches!(
        app.prompt.as_ref(),
        Some(Prompt::Delete { .. } | Prompt::Settle { .. })
    ) {
        match key.code {
            KeyCode::Esc => {
                app.prompt = None;
                app.message = "Cancelled".into();
            }
            KeyCode::Enter => app.submit()?,
            _ => {}
        }
        return Ok(false);
    }
    if app.prompt.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.prompt = None;
                app.input.clear();
            }
            KeyCode::Enter => {
                let prompt = app.prompt.clone();
                let input = app.input.clone();
                if let Err(error) = app.submit() {
                    app.prompt = prompt;
                    app.input = input;
                    return Err(error);
                }
                return Ok(app.should_exit);
            }
            KeyCode::Backspace => {
                app.input.pop();
            }
            KeyCode::Char(ch) => app.input.push(ch),
            _ => {}
        }
        return Ok(false);
    }
    if app.screen == Screen::Reader {
        match key.code {
            KeyCode::Esc | KeyCode::Char('r') => app.screen = Screen::Detail,
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Char('j') | KeyCode::Down => {
                app.reader_scroll = app
                    .reader_scroll
                    .saturating_add(1)
                    .min(app.reader_max_scroll)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.reader_scroll = app.reader_scroll.saturating_sub(1)
            }
            KeyCode::Char('J') => {
                app.reader_scroll = app
                    .reader_scroll
                    .saturating_add(10)
                    .min(app.reader_max_scroll)
            }
            KeyCode::Char('K') => app.reader_scroll = app.reader_scroll.saturating_sub(10),
            KeyCode::PageDown => {
                app.reader_scroll = app
                    .reader_scroll
                    .saturating_add(15)
                    .min(app.reader_max_scroll)
            }
            KeyCode::PageUp => app.reader_scroll = app.reader_scroll.saturating_sub(15),
            KeyCode::Char('g') => app.reader_scroll = 0,
            KeyCode::Char('L') => {
                if let Some(record) = app.current() {
                    app.begin(Prompt::Relink {
                        id: record.id.clone(),
                    });
                }
            }
            KeyCode::Char('e') => {
                if let Some(record) = app.current() {
                    open_editor(&record.spec_path)?;
                }
            }
            _ => {}
        }
        return Ok(false);
    }
    if app.screen == Screen::Detail {
        let actions = app.actions();
        match key.code {
            KeyCode::Esc => {
                app.feedback = None;
                app.screen = Screen::List;
            }
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Char('j') | KeyCode::Down => app.move_milestone(true),
            KeyCode::Char('k') | KeyCode::Up => app.move_milestone(false),
            KeyCode::Char('r') => {
                app.feedback = None;
                app.reader_scroll = 0;
                app.reader_max_scroll = 0;
                app.screen = Screen::Reader;
            }
            KeyCode::Char('x') => {
                if let Some(record) = app.current().filter(|record| record.active_spec_session()) {
                    app.begin(Prompt::Settle {
                        id: record.id.clone(),
                    });
                }
            }
            KeyCode::Char('L') => {
                if let Some(record) = app.current() {
                    app.begin(Prompt::Relink {
                        id: record.id.clone(),
                    });
                }
            }
            KeyCode::Char('e') => {
                if let Some(record) = app.current() {
                    open_editor(&record.spec_path)?;
                }
            }
            KeyCode::Char('o') => {
                if let Some(url) = app.current().and_then(|record| record.jira.url.as_deref()) {
                    open_url(url)?;
                    app.message = "Opened Jira ticket".into();
                }
            }
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') if !actions.is_empty() => {
                app.action_selected = (app.action_selected + 1) % actions.len();
            }
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') if !actions.is_empty() => {
                app.action_selected = (app.action_selected + actions.len() - 1) % actions.len();
            }
            KeyCode::Enter if !actions.is_empty() => {
                start_detail_action(app, actions[app.action_selected])?;
            }
            KeyCode::Char('d') => {
                if let Some(record) = app.current() {
                    app.begin(Prompt::Delete {
                        id: record.id.clone(),
                    });
                }
            }
            _ => {}
        }
        app.refresh()?;
        return Ok(false);
    }
    match key.code {
        KeyCode::Char('s') => super::settings::open(app)?,
        KeyCode::Char('S') => {
            let report = app.store.scan()?;
            super::scan::apply(app, report);
        }
        KeyCode::Char('u') => super::trash::open(app)?,
        KeyCode::Esc => return Ok(true),
        KeyCode::Enter if app.current().is_some() => {
            app.screen = Screen::Detail;
            app.message.clear();
            if let Some(record) = app.current() {
                app.select_milestone(Milestone::next(record));
            }
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.selected = (app.selected + 1).min(app.records.len().saturating_sub(1))
        }
        KeyCode::Char('k') | KeyCode::Up => app.selected = app.selected.saturating_sub(1),
        KeyCode::Char('n') => {
            app.choose_client(ChoicePurpose::NewSpec, launch::available_profiles());
        }
        KeyCode::Char('d') => {
            if let Some(record) = app.current() {
                app.begin(Prompt::Delete {
                    id: record.id.clone(),
                });
            }
        }
        _ => {}
    }
    app.refresh()?;
    Ok(false)
}

fn start_detail_action(app: &mut App, action: DetailAction) -> Result<()> {
    let Some(record) = app.current().cloned() else {
        return Ok(());
    };
    let id = record.id.clone();
    match action {
        DetailAction::Finish if record.title.is_empty() => app.begin(Prompt::FinishTitle { id }),
        DetailAction::Finish => {
            let updated = app.store.update(&id, Change::Finish { title: None })?;
            let _ = launch::rename_tab(&updated);
            app.acknowledge(Milestone::Spec);
        }
        DetailAction::Jira => app.begin(Prompt::Jira { id }),
        DetailAction::Implement => app.begin(Prompt::Agent { id }),
        DetailAction::Pr => app.begin(Prompt::Pr { id }),
        DetailAction::ReadSpec => {
            app.feedback = None;
            app.reader_scroll = 0;
            app.reader_max_scroll = 0;
            app.screen = Screen::Reader;
        }
        DetailAction::RefineSpec => {
            app.choose_client(ChoicePurpose::Refine { id }, launch::available_profiles());
        }
        DetailAction::OpenJira => {
            if let Some(url) = record.jira.url.as_deref() {
                open_url(url)?;
                app.message = "Opened Jira ticket".into();
            }
        }
        DetailAction::UpdateJira => {
            app.begin(Prompt::Jira { id });
            app.input = record.jira.key.unwrap_or_default();
        }
        DetailAction::UpdateImplementation => {
            app.begin(Prompt::Agent { id });
            app.input = record.implementation.agent.unwrap_or_default();
        }
        DetailAction::UpdatePr => {
            app.begin(Prompt::Pr { id });
            app.input = record.pr.url.unwrap_or_default();
        }
        DetailAction::ReviewPr => {
            if let Some(url) = record.pr.url.as_deref() {
                let result = Command::new("open").arg(url).status()?;
                if !result.success() {
                    return Err("Could not open draft PR".into());
                }
                app.message = "Opened draft PR".into();
            }
        }
    }
    Ok(())
}

pub(super) fn handle_mouse(app: &mut App, mouse: MouseEvent, height: u16) -> Result<()> {
    if matches!(
        app.screen,
        Screen::Settings | Screen::Trash | Screen::ScanResult
    ) {
        return Ok(());
    }
    if app.screen == Screen::Reader {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                app.reader_scroll = app
                    .reader_scroll
                    .saturating_add(3)
                    .min(app.reader_max_scroll)
            }
            MouseEventKind::ScrollUp => app.reader_scroll = app.reader_scroll.saturating_sub(3),
            _ => {}
        }
        return Ok(());
    }
    if app.prompt.is_some() || app.choice_selected.is_some() {
        return Ok(());
    }
    if app.screen == Screen::Detail {
        match mouse.kind {
            MouseEventKind::ScrollDown => app.move_milestone(true),
            MouseEventKind::ScrollUp => app.move_milestone(false),
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some((stage, _)) = app.milestone_hitboxes.iter().find(|(_, area)| {
                    (mouse.row == area.y && mouse.column >= area.x && mouse.column < area.right())
                        || (mouse.column >= area.x + 5
                            && mouse.column <= area.x + 9
                            && mouse.row >= area.y.saturating_sub(1)
                            && mouse.row <= area.y + 1)
                }) {
                    app.select_milestone(*stage);
                    return Ok(());
                }
            }
            _ => {}
        }
        if let Some(index) = app.action_hitboxes.iter().position(|area| {
            mouse.row >= area.y
                && mouse.row < area.bottom()
                && mouse.column >= area.x
                && mouse.column < area.x.saturating_add(area.width)
        }) {
            let actions = app.actions();
            app.action_selected = index;
            if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                && let Some(action) = actions.get(index)
            {
                start_detail_action(app, *action)?;
                app.refresh()?;
            }
        }
        return Ok(());
    }
    match mouse.kind {
        MouseEventKind::Moved | MouseEventKind::Down(MouseButton::Left) => {
            let row = mouse.row as usize;
            if row >= 2 && row < height.saturating_sub(3) as usize {
                let index = app.list_offset + row - 2;
                if index < app.records.len() {
                    app.selected = index;
                }
            }
        }
        MouseEventKind::ScrollDown => {
            app.selected = (app.selected + 1).min(app.records.len().saturating_sub(1));
        }
        MouseEventKind::ScrollUp => app.selected = app.selected.saturating_sub(1),
        _ => {}
    }
    Ok(())
}

fn open_editor(path: &std::path::Path) -> Result<()> {
    disable_raw_mode()?;
    crossterm::execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "code".into());
    let result = Command::new(editor).arg(path).status();
    crossterm::execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    enable_raw_mode()?;
    if !result?.success() {
        return Err("Editor failed".into());
    }
    Ok(())
}

fn open_url(url: &str) -> Result<()> {
    if !Command::new("open").arg(url).status()?.success() {
        return Err("Could not open Jira ticket".into());
    }
    Ok(())
}
