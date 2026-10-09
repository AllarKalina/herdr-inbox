//! Prompts and the client choice: the two things that take over the keyboard until they are
//! answered or dismissed. The list shows them in a panel above its shortcuts; the detail
//! view shows prompts beside the selected milestone and the client choice on its own.

use super::state::Tone;
use super::{App, ChoicePurpose, Milestone, Prompt, Screen, chrome, detail, text, theme};
use crate::launch::{self, Options, Profile};
use crate::store::{Change, Record, Result};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use std::path::PathBuf;

/// Rows the list reserves for a prompt or client choice.
pub(super) const PANEL_HEIGHT: u16 = 8;

pub(super) struct Choice {
    pub purpose: ChoicePurpose,
    pub profiles: Vec<Profile>,
    pub selected: usize,
}

#[derive(Default)]
pub(super) struct Modal {
    pub prompt: Option<Prompt>,
    pub input: String,
    pub choice: Option<Choice>,
}

impl Modal {
    pub(super) fn is_open(&self) -> bool {
        self.prompt.is_some() || self.choice.is_some()
    }

    pub(super) fn close(&mut self) {
        self.prompt = None;
        self.input.clear();
        self.choice = None;
    }

    /// Closes a prompt or choice whose item has left the Inbox. Starting a new spec belongs
    /// to no item, so it survives whatever happens to the others.
    pub(super) fn drop_orphans(&mut self, records: &[Record]) {
        let shown = |id: &str| records.iter().any(|record| record.id == id);
        if self
            .prompt
            .as_ref()
            .and_then(Prompt::item)
            .is_some_and(|id| !shown(id))
        {
            self.prompt = None;
            self.input.clear();
        }
        if let Some(Choice {
            purpose: ChoicePurpose::Refine { id },
            ..
        }) = &self.choice
            && !shown(id)
        {
            self.choice = None;
        }
    }
}

impl App {
    pub(super) fn begin(&mut self, prompt: Prompt) {
        self.begin_with(prompt, String::new());
    }

    /// Opens a prompt with its current value ready to edit.
    pub(super) fn begin_with(&mut self, prompt: Prompt, input: String) {
        self.detail.feedback = None;
        self.modal.prompt = Some(prompt);
        self.modal.input = input;
    }

    /// Offers the installed clients, starting on the configured favourite.
    pub(super) fn choose_client(&mut self, purpose: ChoicePurpose, profiles: Vec<Profile>) {
        self.detail.feedback = None;
        if profiles.is_empty() {
            self.modal.choice = None;
            self.notice
                .error("No supported client found (install codex or claude)");
            return;
        }
        let preferred = self.config.preferred_client.as_deref();
        let selected = profiles
            .iter()
            .position(|profile| Some(profile.id()) == preferred)
            .unwrap_or(0);
        self.notice.clear();
        self.modal.choice = Some(Choice {
            purpose,
            profiles,
            selected,
        });
    }
}

/// The breadcrumb while a modal belongs to a flow of its own.
pub(super) fn crumbs(app: &App) -> Option<Vec<String>> {
    match (&app.modal.choice, &app.modal.prompt) {
        (Some(choice), _) => Some(match choice.purpose {
            ChoicePurpose::NewSpec => vec!["New spec".into()],
            ChoicePurpose::Refine { .. } => {
                let mut crumbs = detail::crumbs(app);
                crumbs.push("Refine".into());
                crumbs
            }
        }),
        (None, Some(prompt)) if prompt.item().is_none() => Some(vec!["New spec".into()]),
        _ => None,
    }
}

/// Handles a key while a modal is open. `None` means no modal is open.
pub(super) fn handle_key(app: &mut App, key: KeyEvent) -> Option<Result<bool>> {
    if app.modal.choice.is_some() {
        return Some(choice_key(app, key).map(|()| false));
    }
    let prompt = app.modal.prompt.clone()?;
    Some(prompt_key(app, prompt, key).map(|()| false))
}

fn choice_key(app: &mut App, key: KeyEvent) -> Result<()> {
    let Some(choice) = &mut app.modal.choice else {
        return Ok(());
    };
    match key.code {
        KeyCode::Esc => app.modal.choice = None,
        KeyCode::Char('j') | KeyCode::Down => {
            choice.selected = (choice.selected + 1).min(choice.profiles.len() - 1)
        }
        KeyCode::Char('k') | KeyCode::Up => choice.selected = choice.selected.saturating_sub(1),
        KeyCode::Enter => {
            let profile = choice.profiles[choice.selected];
            match choice.purpose.clone() {
                ChoicePurpose::NewSpec => {
                    app.modal.choice = None;
                    let workspace = app.config.workspace.clone();
                    app.begin_with(Prompt::LaunchWorkspace { profile }, workspace);
                }
                ChoicePurpose::Refine { id } => {
                    let mut options = Options::for_profile(profile);
                    options.workspace = app.config.workspace.clone();
                    launch::refine(&app.store, &id, options)?;
                    app.modal.choice = None;
                    // The session's tab now has focus; the popup would only hide it.
                    app.should_exit = true;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn prompt_key(app: &mut App, prompt: Prompt, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.modal.prompt = None;
            app.modal.input.clear();
            if prompt.is_confirmation() {
                app.notice.info("Cancelled");
            }
        }
        KeyCode::Enter => {
            let input = app.modal.input.clone();
            if let Err(error) = submit(app) {
                // A rejected answer stays on screen to be corrected; a refused
                // confirmation has nothing to correct.
                if !prompt.is_confirmation() {
                    app.modal.prompt = Some(prompt);
                    app.modal.input = input;
                }
                return Err(error);
            }
        }
        KeyCode::Backspace if !prompt.is_confirmation() => {
            app.modal.input.pop();
        }
        KeyCode::Char(ch) if !prompt.is_confirmation() => app.modal.input.push(ch),
        _ => {}
    }
    Ok(())
}

/// Carries out the open prompt with what was typed.
fn submit(app: &mut App) -> Result<()> {
    let Some(prompt) = app.modal.prompt.take() else {
        return Ok(());
    };
    let value = std::mem::take(&mut app.modal.input).trim().to_string();
    let filled = (!value.is_empty()).then(|| value.clone());
    match prompt {
        Prompt::LaunchWorkspace { profile } => app.begin(Prompt::LaunchRepo {
            profile,
            workspace: filled.unwrap_or_else(|| app.config.workspace.clone()),
        }),
        Prompt::LaunchRepo { profile, workspace } => app.begin(Prompt::LaunchSpec {
            profile,
            workspace,
            repo: filled.map(PathBuf::from),
        }),
        Prompt::LaunchSpec {
            profile,
            workspace,
            repo,
        } => app.begin(Prompt::LaunchTopic {
            profile,
            workspace,
            repo,
            spec: filled.as_deref().map(text::typed_path).transpose()?,
        }),
        Prompt::LaunchTopic {
            profile,
            workspace,
            repo,
            spec,
        } => {
            let options = Options {
                workspace,
                repo,
                topic: value,
                ..Options::for_profile(profile)
            };
            launch::start(&app.store, options, spec)?;
            // The session's tab now has focus; the popup would only hide it.
            app.should_exit = true;
        }
        Prompt::Settle { id } => {
            app.store.settle(&id)?;
            app.notice
                .success("Session settled; its spec and progress stay unchanged");
        }
        Prompt::Archive { id } => {
            app.store.archive(&id)?;
            app.notice.info("Item archived");
            app.screen = Screen::List;
        }
        Prompt::FinishTitle { id } if filled.is_some() => {
            let record = app.store.update(&id, Change::Finish { title: filled })?;
            let _ = launch::rename_tab(&record);
            app.acknowledge(Milestone::Spec);
        }
        Prompt::Jira { id } if filled.is_some() => {
            let url = app.store.get(&id)?.jira.url.unwrap_or_default();
            app.begin_with(Prompt::JiraUrl { id, key: value }, url);
        }
        Prompt::JiraUrl { id, key } => {
            app.store.update(&id, Change::Jira { key, url: filled })?;
            app.acknowledge(Milestone::Jira);
        }
        Prompt::Agent { id } => {
            let branch = app.store.get(&id)?.implementation.branch;
            app.begin_with(
                Prompt::Branch { id, agent: filled },
                branch.unwrap_or_default(),
            );
        }
        Prompt::Branch { id, agent } => {
            let change = Change::Implement {
                agent,
                branch: filled,
            };
            app.store.update(&id, change)?;
            app.acknowledge(Milestone::Dev);
        }
        Prompt::Pr { id } if filled.is_some() => {
            app.store.update(&id, Change::Pr { url: value })?;
            app.acknowledge(Milestone::Pr);
        }
        // A required answer left empty changes nothing.
        Prompt::FinishTitle { .. } | Prompt::Jira { .. } | Prompt::Pr { .. } => {
            app.notice.info("Cancelled")
        }
    }
    Ok(())
}

const CHOICE_HINTS: &str = "j/k choose · Enter continue · Esc cancel";

fn choice_hints(width: u16) -> &'static str {
    if usize::from(width) >= CHOICE_HINTS.chars().count() {
        CHOICE_HINTS
    } else {
        "j/k · Enter continue · Esc cancel"
    }
}

/// The shortcut line while a modal is open in the list.
pub(super) fn hints(app: &App, width: u16) -> Option<&'static str> {
    if app.modal.choice.is_some() {
        return Some(choice_hints(width));
    }
    Some(match app.modal.prompt.as_ref()? {
        Prompt::Archive { .. } => "Enter archive this item · Esc cancel",
        _ => "Enter save · Esc cancel",
    })
}

/// Draws the open modal in the panel the list reserves for it.
pub(super) fn draw_panel(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    if let Some(choice) = &app.modal.choice {
        draw_choices(frame, choice, area);
    } else if let Some(Prompt::Archive { id }) = &app.modal.prompt {
        let detail = match app.record(id) {
            Some(record) => format!(
                "ARCHIVE ITEM\n{}\nID: {}\nSpec: {}\nLinked spec stays in place.\n\
                 Open agent tabs are not closed.",
                record.display_title(),
                record.id,
                record.spec_path.display(),
            ),
            None => "Item disappeared; press Esc to cancel.".into(),
        };
        let block = Block::default()
            .title(" Confirm archive ")
            .borders(Borders::ALL);
        frame.render_widget(Paragraph::new(detail).block(block), area);
    } else if let Some(prompt) = &app.modal.prompt {
        let block = Block::default().title(" Action ").borders(Borders::ALL);
        let line = format!("{}: {}█", prompt.label(), app.modal.input);
        frame.render_widget(Paragraph::new(line).block(block), area);
    }
}

fn draw_choices(frame: &mut ratatui::Frame, choice: &Choice, area: Rect) {
    let title = match choice.purpose {
        ChoicePurpose::Refine { .. } => " Refine spec · Choose client ",
        ChoicePurpose::NewSpec => " Choose client ",
    };
    let items = choice
        .profiles
        .iter()
        .map(|profile| ListItem::new(profile.label()));
    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(theme::selection());
    let mut state = ListState::default().with_selected(Some(choice.selected));
    frame.render_stateful_widget(list, area, &mut state);
}

/// Draws the client choice as a view of its own, in place of the detail view.
pub(super) fn draw_choice(frame: &mut ratatui::Frame, app: &App) {
    let Some(choice) = &app.modal.choice else {
        return;
    };
    let area = frame.area();
    let width = area.width.saturating_sub(4).min(52);
    let height = area.height.saturating_sub(2).min(9);
    let popup = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    draw_choices(
        frame,
        choice,
        Rect::new(popup.x, popup.y, popup.width, 5.min(popup.height)),
    );
    // A failed launch explains itself directly under the choice it came from.
    if app.notice.tone() == Tone::Error {
        frame.render_widget(
            Paragraph::new(app.notice.text())
                .style(theme::error())
                .wrap(Wrap { trim: false }),
            Rect::new(
                popup.x + 1,
                popup.y + 5,
                popup.width.saturating_sub(2),
                popup.height.saturating_sub(6),
            ),
        );
    }
    let footer = chrome::footer_area(area);
    chrome::draw_footer(frame, choice_hints(footer.width));
}

/// The open prompt as lines for the detail view's milestone controls.
pub(super) fn rail_lines(app: &App, width: u16) -> Vec<Line<'static>> {
    let Some(prompt) = &app.modal.prompt else {
        return Vec::new();
    };
    let width = usize::from(width);
    let lines = if matches!(prompt, Prompt::Settle { .. }) {
        vec![
            "Session ended?".into(),
            "No tabs closed.".into(),
            "Enter settle".into(),
            "Esc cancel".into(),
        ]
    } else {
        // Long input scrolls so the cursor stays visible.
        let count = app.modal.input.chars().count();
        let tail: String = app
            .modal
            .input
            .chars()
            .skip(count.saturating_sub(width.saturating_sub(1)))
            .collect();
        vec![
            text::fit_label(prompt.label(), width),
            format!("{tail}█"),
            "Enter save · Esc cancel".into(),
        ]
    };
    lines.into_iter().map(Line::from).collect()
}
