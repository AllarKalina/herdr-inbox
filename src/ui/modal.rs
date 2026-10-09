//! Prompts and the client choice: the two things that take over the keyboard until they are
//! answered or dismissed. The list shows them in a panel above its shortcuts; the detail
//! view shows prompts beside the selected milestone and the client choice on its own.

use super::{App, Milestone, Screen, detail, text};
use crate::launch::{self, Options, Profile};
use crate::store::{Change, Record, Result};
use crossterm::event::{KeyCode, KeyEvent};
use std::path::PathBuf;

mod view;
pub(super) use view::{draw_choice, draw_panel, hints, rail_lines};

/// A question the user is answering. Launch prompts form a chain that ends by starting a
/// session; the others act on one item.
#[derive(Clone)]
pub(super) enum Prompt {
    LaunchWorkspace {
        profile: Profile,
    },
    LaunchRepo {
        profile: Profile,
        workspace: String,
    },
    LaunchSpec {
        profile: Profile,
        workspace: String,
        repo: Option<PathBuf>,
    },
    LaunchTopic {
        profile: Profile,
        workspace: String,
        repo: Option<PathBuf>,
        spec: Option<PathBuf>,
    },
    Settle {
        id: String,
    },
    FinishTitle {
        id: String,
    },
    Jira {
        id: String,
    },
    JiraUrl {
        id: String,
        key: String,
    },
    Agent {
        id: String,
    },
    Branch {
        id: String,
        agent: Option<String>,
    },
    Pr {
        id: String,
    },
    Archive {
        id: String,
    },
}

impl Prompt {
    /// The item this prompt acts on; launch prompts create a new one.
    pub(super) fn item(&self) -> Option<&str> {
        match self {
            Self::Settle { id }
            | Self::FinishTitle { id }
            | Self::Jira { id }
            | Self::JiraUrl { id, .. }
            | Self::Agent { id }
            | Self::Branch { id, .. }
            | Self::Pr { id }
            | Self::Archive { id } => Some(id),
            Self::LaunchWorkspace { .. }
            | Self::LaunchRepo { .. }
            | Self::LaunchSpec { .. }
            | Self::LaunchTopic { .. } => None,
        }
    }

    /// A yes-or-no question: Enter confirms and typing does nothing.
    pub(super) fn is_confirmation(&self) -> bool {
        matches!(self, Self::Archive { .. } | Self::Settle { .. })
    }

    pub(super) fn label(&self) -> &'static str {
        match self {
            Self::LaunchWorkspace { .. } => "Herdr workspace",
            Self::LaunchSpec { .. } => "Spec path (blank = first source folder)",
            Self::Settle { .. } => "Confirm session has ended",
            Self::LaunchRepo { .. } => "Repo path (blank for workspace cwd)",
            Self::LaunchTopic { .. } => "Grilling topic (optional)",
            Self::FinishTitle { .. } => "Finished spec title",
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
