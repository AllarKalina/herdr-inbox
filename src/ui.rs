mod detail;
mod draw;
mod input;
mod list;
mod milestone;
mod picker;
mod progress;
mod scan;
mod settings;
mod submit;
#[cfg(test)]
mod tests;
mod trash;
mod tree;
use input::{handle_key, handle_mouse};
use milestone::Milestone;

use crate::launch::Profile;
use crate::store::{Record, Result, Store};
use crossterm::event::{self, Event};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use std::io::{self, stdout};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone)]
enum Prompt {
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
    Relink {
        id: String,
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
    Delete {
        id: String,
    },
}

impl Prompt {
    fn label(&self) -> &'static str {
        match self {
            Self::LaunchWorkspace { .. } => "Herdr workspace",
            Self::LaunchSpec { .. } => "Spec path (blank = first source folder)",
            Self::Relink { .. } => "Existing spec file to relink",
            Self::Settle { .. } => "Confirm session has ended",
            Self::LaunchRepo { .. } => "Repo path (blank for workspace cwd)",
            Self::LaunchTopic { .. } => "Grilling topic (optional)",
            Self::FinishTitle { .. } => "Finished spec title",
            Self::Jira { .. } => "Jira key",
            Self::JiraUrl { .. } => "Jira URL (optional)",
            Self::Agent { .. } => "Agent name (optional)",
            Self::Branch { .. } => "Branch (optional)",
            Self::Pr { .. } => "Draft PR URL",
            Self::Delete { .. } => "Confirm archive",
        }
    }
}

struct App {
    store: Store,
    records: Vec<Record>,
    selected: usize,
    tree: tree::Tree,
    list_area: Rect,
    screen: Screen,
    action_selected: usize,
    action_hitboxes: Vec<Rect>,
    milestone_selected: Milestone,
    milestone_hitboxes: Vec<(Milestone, Rect)>,
    reader_scroll: u16,
    reader_max_scroll: u16,
    list_offset: usize,
    prompt: Option<Prompt>,
    input: String,
    choices: Vec<Profile>,
    choice_selected: Option<usize>,
    choice_purpose: ChoicePurpose,
    feedback: Option<MilestoneFeedback>,
    message: String,
    should_exit: bool,
    settings: settings::SettingsView,
    trash: Vec<Record>,
    trash_selected: usize,
    trash_return: Screen,
    scan_issues: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    List,
    Detail,
    Reader,
    Settings,
    Trash,
    ScanResult,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ChoicePurpose {
    NewSpec,
    Refine { id: String },
}

struct MilestoneFeedback {
    milestone: Milestone,
}

impl MilestoneFeedback {
    fn text(&self, compact: bool) -> &'static str {
        match (self.milestone, compact) {
            (Milestone::Spec, false) => "Spec sealed",
            (Milestone::Jira, false) => "Jira bound",
            (Milestone::Dev, false) => "Dev quest logged",
            (Milestone::Pr, false) => "Draft PR bound",
            (Milestone::Spec, true) => "Sealed",
            (Milestone::Jira | Milestone::Pr, true) => "Bound",
            (Milestone::Dev, true) => "Logged",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DetailAction {
    Finish,
    Jira,
    Implement,
    Pr,
    ReviewPr,
    ReadSpec,
    RefineSpec,
    OpenJira,
    UpdateJira,
    UpdateImplementation,
    UpdatePr,
}

impl DetailAction {
    fn label(self) -> &'static str {
        match self {
            Self::Finish => "Seal the spec",
            Self::Jira => "Bind Jira ticket",
            Self::Implement => "Log dev quest",
            Self::Pr => "Bind draft PR",
            Self::ReviewPr => "Review draft PR",
            Self::ReadSpec => "Read the scroll",
            Self::RefineSpec => "Refine the spec",
            Self::OpenJira => "Visit Jira ticket",
            Self::UpdateJira => "Update Jira link",
            Self::UpdateImplementation => "Update dev quest",
            Self::UpdatePr => "Update PR link",
        }
    }
}

fn display_implementation_stage(record: &Record) -> &str {
    if record.implementation.status == "draft_pr" {
        "done"
    } else {
        record.implementation_stage()
    }
}

impl App {
    fn new(store: Store) -> Result<Self> {
        let settings = store.settings()?;
        let report = store.scan()?;
        let records = store.list()?;
        let first_use = settings.sources.is_empty() && records.is_empty();
        let mut tree = tree::Tree::default();
        tree.rebuild(&records, &settings.sources, &store.path().join("specs"));
        Ok(Self {
            store,
            records,
            selected: 0,
            tree,
            list_area: Rect::default(),
            screen: if first_use {
                Screen::Settings
            } else if !report.issues.is_empty() {
                Screen::ScanResult
            } else {
                Screen::List
            },
            action_selected: 0,
            action_hitboxes: Vec::new(),
            milestone_selected: Milestone::Spec,
            milestone_hitboxes: Vec::new(),
            reader_scroll: 0,
            reader_max_scroll: 0,
            list_offset: 0,
            prompt: None,
            input: String::new(),
            choices: Vec::new(),
            choice_selected: None,
            choice_purpose: ChoicePurpose::NewSpec,
            feedback: None,
            message: String::new(),
            should_exit: false,
            settings: settings::SettingsView::new(settings, first_use),
            trash: Vec::new(),
            trash_selected: 0,
            trash_return: Screen::List,
            scan_issues: report.issues.clone(),
        })
    }

    fn refresh(&mut self) -> Result<()> {
        let old_next = self.current().map(Milestone::next);
        let id = self
            .records
            .get(self.selected)
            .map(|record| record.id.clone());
        self.records = self.store.list()?;
        let still_present = id
            .as_ref()
            .and_then(|id| self.records.iter().position(|record| &record.id == id));
        if still_present.is_none() && matches!(self.screen, Screen::Detail | Screen::Reader) {
            self.screen = Screen::List;
            self.feedback = None;
            self.message = "Item no longer in inbox".into();
        }
        self.selected =
            still_present.unwrap_or(self.selected.min(self.records.len().saturating_sub(1)));
        self.tree.rebuild(
            &self.records,
            &self.store.settings()?.sources,
            &self.store.path().join("specs"),
        );
        if matches!(self.screen, Screen::Detail | Screen::Reader) {
            self.tree.focus_record(self.selected);
        } else {
            self.sync_tree_selection();
        }
        let new_next = self.current().map(Milestone::next);
        if old_next != new_next
            && old_next == Some(self.milestone_selected)
            && let Some(next) = new_next
        {
            self.focus_milestone(next);
        }
        self.action_selected = self
            .action_selected
            .min(self.actions().len().saturating_sub(1));
        Ok(())
    }

    fn current(&self) -> Option<&Record> {
        if self.screen == Screen::List {
            self.tree
                .selected_record()
                .and_then(|index| self.records.get(index))
        } else {
            self.records.get(self.selected)
        }
    }

    fn sync_tree_selection(&mut self) {
        if let Some(index) = self.tree.selected_record() {
            self.selected = index;
        }
    }

    fn actions(&self) -> Vec<DetailAction> {
        self.current()
            .map_or_else(Vec::new, |record| self.milestone_selected.actions(record))
    }

    fn select_milestone(&mut self, milestone: Milestone) {
        self.feedback = None;
        self.focus_milestone(milestone);
    }

    fn focus_milestone(&mut self, milestone: Milestone) {
        self.milestone_selected = milestone;
        self.action_selected = 0;
        self.action_hitboxes.clear();
    }

    fn move_milestone(&mut self, forward: bool) {
        let index = Milestone::ALL
            .iter()
            .position(|stage| *stage == self.milestone_selected)
            .unwrap_or(0);
        let next = if forward {
            (index + 1).min(3)
        } else {
            index.saturating_sub(1)
        };
        self.select_milestone(Milestone::ALL[next]);
    }

    fn begin(&mut self, prompt: Prompt) {
        self.feedback = None;
        self.prompt = Some(prompt);
        self.input.clear();
    }

    fn choose_client(&mut self, purpose: ChoicePurpose, choices: Vec<Profile>) {
        self.feedback = None;
        self.choice_purpose = purpose;
        self.choices = choices;
        self.choice_selected = if self.choices.is_empty() {
            None
        } else {
            Some(
                self.store
                    .settings()
                    .ok()
                    .and_then(|settings| settings.preferred_client)
                    .and_then(|id| self.choices.iter().position(|profile| profile.id() == id))
                    .unwrap_or(0),
            )
        };
        self.message = if self.choices.is_empty() {
            "No supported client found (install codex or claude)".into()
        } else {
            String::new()
        };
    }

    fn acknowledge(&mut self, milestone: Milestone) {
        self.feedback = Some(MilestoneFeedback { milestone });
        self.message.clear();
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
