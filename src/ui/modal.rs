//! Prompts and the client choice: the two things that take over the keyboard until they are
//! answered or dismissed. The list shows them in a panel above its shortcuts; the detail
//! view shows prompts beside the selected milestone and the client choice on its own.

use super::{App, Milestone, Screen, detail};
use crate::launch::{self, Options, Profile};
use crate::store::{Change, Record, Result};
use crossterm::event::{KeyCode, KeyEvent};

mod view;
pub(super) use view::{draw_choice, draw_panel, hints, panel_height, rail_lines};

/// A question about one item that the user is answering.
#[derive(Clone)]
pub(super) enum Prompt {
    Settle { id: String },
    FinishTitle { id: String },
    JiraParent { id: String },
    Jira { id: String },
    JiraUrl { id: String, key: String },
    Agent { id: String },
    Branch { id: String, agent: Option<String> },
    Pr { id: String },
    Archive { id: String },
}

impl Prompt {
    /// The item this prompt acts on.
    pub(super) fn item(&self) -> &str {
        match self {
            Self::Settle { id }
            | Self::FinishTitle { id }
            | Self::JiraParent { id }
            | Self::Jira { id }
            | Self::JiraUrl { id, .. }
            | Self::Agent { id }
            | Self::Branch { id, .. }
            | Self::Pr { id }
            | Self::Archive { id } => id,
        }
    }

    /// A yes-or-no question: Enter confirms and typing does nothing.
    pub(super) fn is_confirmation(&self) -> bool {
        matches!(self, Self::Archive { .. } | Self::Settle { .. })
    }

    pub(super) fn label(&self) -> &'static str {
        match self {
            Self::Settle { .. } => "Confirm session has ended",
            Self::FinishTitle { .. } => "Finished spec title",
            Self::JiraParent { .. } => "Parent key (optional)",
            Self::Jira { .. } => "Jira key",
            Self::JiraUrl { .. } => "Jira URL (optional)",
            Self::Agent { .. } => "Agent name (optional)",
            Self::Branch { .. } => "Branch (optional)",
            Self::Pr { .. } => "Draft PR URL",
            Self::Archive { .. } => "Confirm archive",
        }
    }
}

/// Why a client is being chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ChoicePurpose {
    NewSpec,
    Refine { id: String },
}

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
            .is_some_and(|prompt| !shown(prompt.item()))
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

/// The breadcrumb while the client choice is open: it is a flow of its own.
pub(super) fn crumbs(app: &App) -> Option<Vec<String>> {
    Some(match app.modal.choice.as_ref()?.purpose {
        ChoicePurpose::NewSpec => vec!["New spec".into()],
        ChoicePurpose::Refine { .. } => {
            let mut crumbs = detail::crumbs(app);
            crumbs.push("Refine".into());
            crumbs
        }
    })
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
    let last = choice.profiles.len() - 1;
    // A new spec also takes a topic, so letters type; the arrows and Tab pick the client.
    let typing = choice.purpose == ChoicePurpose::NewSpec;
    match key.code {
        KeyCode::Esc => app.modal.close(),
        KeyCode::Down | KeyCode::Tab => choice.selected = (choice.selected + 1).min(last),
        KeyCode::Up | KeyCode::BackTab => choice.selected = choice.selected.saturating_sub(1),
        KeyCode::Char('j') if !typing => choice.selected = (choice.selected + 1).min(last),
        KeyCode::Char('k') if !typing => choice.selected = choice.selected.saturating_sub(1),
        KeyCode::Char(ch) if typing => app.modal.input.push(ch),
        KeyCode::Backspace if typing => {
            app.modal.input.pop();
        }
        KeyCode::Enter => {
            let options = Options {
                topic: app.modal.input.trim().to_owned(),
                ..Options::for_profile(choice.profiles[choice.selected])
            };
            // A failed launch leaves the choice and the typed topic in place to retry.
            match &choice.purpose {
                ChoicePurpose::NewSpec => launch::start(&app.store, options)?,
                ChoicePurpose::Refine { id } => launch::refine(&app.store, id, options)?,
            };
            app.modal.close();
            // The session's tab now has focus; the popup would only hide it.
            app.should_exit = true;
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
            let finished = Change::Finish {
                title: filled,
                spec: None,
            };
            let record = app.store.update(&id, finished)?;
            let _ = launch::rename_tab(&record);
            app.acknowledge(Milestone::Spec);
        }
        Prompt::JiraParent { id } => {
            launch::ticket(&app.store, &id, filled)?;
            // The agent's tab now has focus; the popup would only hide it.
            app.should_exit = true;
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
