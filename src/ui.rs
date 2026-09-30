mod detail;
mod draw;
mod input;
#[cfg(test)]
mod tests;
use input::{handle_key, handle_mouse};

use crate::launch::{self, DEFAULT_WORKSPACE, Options, Profile};
use crate::store::{Change, Record, Result, Store};
use crossterm::event::{self, Event};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{self, stdout};
use std::path::PathBuf;
use std::time::Duration;

enum Prompt {
    LaunchWorkspace {
        profile: Profile,
    },
    LaunchRepo {
        profile: Profile,
        workspace: String,
    },
    LaunchTopic {
        profile: Profile,
        workspace: String,
        repo: Option<PathBuf>,
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
    Delete {
        id: String,
    },
}

impl Prompt {
    fn label(&self) -> &'static str {
        match self {
            Self::LaunchWorkspace { .. } => "Workspace [ai-boiler-room]",
            Self::LaunchRepo { .. } => "Repo path (blank for workspace cwd)",
            Self::LaunchTopic { .. } => "Grilling topic (optional)",
            Self::FinishTitle { .. } => "Finished spec title",
            Self::Jira { .. } => "Jira key",
            Self::JiraUrl { .. } => "Jira URL (optional)",
            Self::Agent { .. } => "Agent name (optional)",
            Self::Branch { .. } => "Branch (optional)",
            Self::Pr { .. } => "Draft PR URL",
            Self::Delete { .. } => "Confirm deletion",
        }
    }
}

struct App {
    store: Store,
    records: Vec<Record>,
    selected: usize,
    screen: Screen,
    action_selected: usize,
    reader_scroll: u16,
    reader_max_scroll: u16,
    list_offset: usize,
    prompt: Option<Prompt>,
    input: String,
    choices: Vec<Profile>,
    choice_selected: Option<usize>,
    message: String,
    should_exit: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    List,
    Detail,
    Reader,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DetailAction {
    Finish,
    Jira,
    Implement,
    Pr,
    ReviewPr,
}

impl DetailAction {
    fn label(self) -> &'static str {
        match self {
            Self::Finish => "Finish spec",
            Self::Jira => "Link Jira",
            Self::Implement => "Start implementation",
            Self::Pr => "Link draft PR",
            Self::ReviewPr => "Review draft PR",
        }
    }
}

fn detail_actions(record: &Record) -> Vec<DetailAction> {
    if record.spec == "in_progress" {
        return vec![DetailAction::Finish];
    }
    let mut actions = Vec::new();
    if record.jira.status == "ready" {
        actions.push(DetailAction::Jira);
    }
    if record.implementation.status == "ready" {
        actions.push(DetailAction::Implement);
    }
    if record.pr.status == "waiting" && record.implementation.status == "in_progress" {
        actions.push(DetailAction::Pr);
    }
    if record.pr.status == "draft" && record.pr.url.is_some() {
        actions.push(DetailAction::ReviewPr);
    }
    actions
}

impl App {
    fn new(store: Store) -> Result<Self> {
        let records = store.list()?;
        Ok(Self {
            store,
            records,
            selected: 0,
            screen: Screen::List,
            action_selected: 0,
            reader_scroll: 0,
            reader_max_scroll: 0,
            list_offset: 0,
            prompt: None,
            input: String::new(),
            choices: Vec::new(),
            choice_selected: None,
            message: String::new(),
            should_exit: false,
        })
    }

    fn refresh(&mut self) -> Result<()> {
        let id = self
            .records
            .get(self.selected)
            .map(|record| record.id.clone());
        self.records = self.store.list()?;
        let still_present = id
            .as_ref()
            .and_then(|id| self.records.iter().position(|record| &record.id == id));
        if still_present.is_none() && self.screen != Screen::List {
            self.screen = Screen::List;
            self.message = "Item no longer in inbox".into();
        }
        self.selected =
            still_present.unwrap_or(self.selected.min(self.records.len().saturating_sub(1)));
        self.action_selected = self.action_selected.min(
            self.current()
                .map_or(0, |record| detail_actions(record).len().saturating_sub(1)),
        );
        Ok(())
    }

    fn current(&self) -> Option<&Record> {
        self.records.get(self.selected)
    }

    fn begin(&mut self, prompt: Prompt) {
        self.prompt = Some(prompt);
        self.input.clear();
    }

    fn submit(&mut self) -> Result<()> {
        let Some(prompt) = self.prompt.take() else {
            return Ok(());
        };
        let value = std::mem::take(&mut self.input).trim().to_string();
        match prompt {
            Prompt::LaunchWorkspace { profile } => self.begin(Prompt::LaunchRepo {
                profile,
                workspace: if value.is_empty() {
                    DEFAULT_WORKSPACE.into()
                } else {
                    value
                },
            }),
            Prompt::LaunchRepo { profile, workspace } => self.begin(Prompt::LaunchTopic {
                profile,
                workspace,
                repo: nonempty(value).map(PathBuf::from),
            }),
            Prompt::LaunchTopic {
                profile,
                workspace,
                repo,
            } => {
                let options = Options {
                    workspace,
                    repo,
                    topic: value,
                    ..Options::for_profile(profile)
                };
                let record = launch::start(&self.store, options)?;
                self.message = format!("Launched spec session {}", record.id);
                self.should_exit = true;
            }
            Prompt::FinishTitle { id } if !value.is_empty() => {
                let record = self
                    .store
                    .update(&id, Change::Finish { title: Some(value) })?;
                let _ = launch::rename_tab(&record);
                self.message = "Spec done; Jira and handoff ready".into();
            }
            Prompt::Jira { id } if !value.is_empty() => {
                self.begin(Prompt::JiraUrl { id, key: value });
            }
            Prompt::JiraUrl { id, key } => {
                self.store.update(
                    &id,
                    Change::Jira {
                        key,
                        url: nonempty(value),
                    },
                )?;
                self.message = "Jira linked".into();
            }
            Prompt::Agent { id } => self.begin(Prompt::Branch {
                id,
                agent: nonempty(value),
            }),
            Prompt::Branch { id, agent } => {
                self.store.update(
                    &id,
                    Change::Implement {
                        agent,
                        branch: nonempty(value),
                    },
                )?;
                self.message = "Implementation started".into();
            }
            Prompt::Pr { id } if !value.is_empty() => {
                self.store.update(&id, Change::Pr { url: value })?;
                self.message = "Draft PR linked".into();
            }
            Prompt::Delete { id } => {
                self.store.delete(&id)?;
                self.message = "Item moved to local Trash".into();
                self.screen = Screen::List;
            }
            _ => self.message = "Cancelled".into(),
        }
        self.refresh()?;
        Ok(())
    }
}

fn nonempty(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

pub fn run(store: Store) -> Result<()> {
    enable_raw_mode()?;
    crossterm::execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let result = run_loop(&mut terminal, store);
    disable_raw_mode()?;
    crossterm::execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
    result
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, store: Store) -> Result<()> {
    let mut app = App::new(store)?;
    loop {
        app.refresh()?;
        terminal.draw(|frame| draw::draw(frame, &mut app))?;
        if event::poll(Duration::from_secs(1))? {
            match event::read()? {
                Event::Key(key) => match handle_key(&mut app, key) {
                    Ok(true) => break,
                    Ok(false) => {}
                    Err(error) => app.message = error.to_string(),
                },
                Event::Mouse(mouse) => {
                    if let Err(error) = handle_mouse(&mut app, mouse, terminal.size()?.height) {
                        app.message = error.to_string();
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}
