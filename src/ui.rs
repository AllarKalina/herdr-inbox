use crate::store::{Change, Record, Result, Store};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use std::io::{self, stdout};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

enum Prompt {
    Title,
    Repo { title: String },
    Jira { id: String },
    Agent { id: String },
    Branch { id: String, agent: Option<String> },
    Pr { id: String },
}

impl Prompt {
    fn label(&self) -> &'static str {
        match self {
            Self::Title => "New spec title",
            Self::Repo { .. } => "Repo path (blank for none)",
            Self::Jira { .. } => "Jira key",
            Self::Agent { .. } => "Agent name (optional)",
            Self::Branch { .. } => "Branch (optional)",
            Self::Pr { .. } => "Draft PR URL",
        }
    }
}

struct App {
    store: Store,
    records: Vec<Record>,
    selected: usize,
    prompt: Option<Prompt>,
    input: String,
    message: String,
}

impl App {
    fn new(store: Store) -> Result<Self> {
        let records = store.list()?;
        Ok(Self {
            store,
            records,
            selected: 0,
            prompt: None,
            input: String::new(),
            message: String::new(),
        })
    }

    fn refresh(&mut self) -> Result<()> {
        let id = self
            .records
            .get(self.selected)
            .map(|record| record.id.clone());
        self.records = self.store.list()?;
        self.selected = id
            .and_then(|id| self.records.iter().position(|record| record.id == id))
            .unwrap_or(self.selected.min(self.records.len().saturating_sub(1)));
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
            Prompt::Title if !value.is_empty() => self.begin(Prompt::Repo { title: value }),
            Prompt::Repo { title } => {
                let repo = if value.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(value))
                };
                let record = self.store.start(&title, repo, None)?;
                self.message = format!("Started {}", record.id);
            }
            Prompt::Jira { id } if !value.is_empty() => {
                self.store.update(
                    &id,
                    Change::Jira {
                        key: value,
                        url: None,
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
            _ => self.message = "Cancelled".into(),
        }
        self.refresh()?;
        Ok(())
    }
}

fn nonempty(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(8),
            Constraint::Length(2),
        ])
        .split(frame.area());
    let rows: Vec<ListItem> = app
        .records
        .iter()
        .map(|record| {
            ListItem::new(format!(
                "{:<7} {:<7} {:<8} {:<6} {}",
                short(&record.spec),
                short(&record.jira.status),
                short(&record.implementation.status),
                short(&record.pr.status),
                record.title,
            ))
        })
        .collect();
    let list = List::new(rows)
        .block(
            Block::default()
                .title(format!(" Inbox · {} items ", app.records.len()))
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let mut state = ListState::default().with_selected(app.current().map(|_| app.selected));
    frame.render_stateful_widget(list, areas[0], &mut state);

    let detail = if let Some(record) = app.current() {
        format!(
            "ID: {}\nRepo: {}\nSpec: {}\nJira: {}  Branch: {}\nPR: {}\nNext: {}",
            record.id,
            record
                .repo
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "—".into()),
            record.spec_path.display(),
            record.jira.key.as_deref().unwrap_or("—"),
            record.implementation.branch.as_deref().unwrap_or("—"),
            record.pr.url.as_deref().unwrap_or("—"),
            record.next_actions().join(", "),
        )
    } else {
        "No items yet. Press n to start a spec.".into()
    };
    frame.render_widget(
        Paragraph::new(detail).block(Block::default().title(" Selected ").borders(Borders::ALL)),
        areas[1],
    );
    let footer = if let Some(prompt) = &app.prompt {
        format!(
            "{}: {}█    Enter save · Esc cancel",
            prompt.label(),
            app.input
        )
    } else if !app.message.is_empty() {
        app.message.clone()
    } else {
        "n new · e edit · f finish · J Jira · i implement · p PR · j/k move · q quit".into()
    };
    frame.render_widget(Paragraph::new(footer), areas[2]);
}

fn short(value: &str) -> &str {
    match value {
        "in_progress" => "working",
        "draft_pr" => "draft",
        other => other,
    }
}

fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    if key.kind != KeyEventKind::Press {
        return Ok(false);
    }
    if app.prompt.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.prompt = None;
                app.input.clear();
            }
            KeyCode::Enter => app.submit()?,
            KeyCode::Backspace => {
                app.input.pop();
            }
            KeyCode::Char(ch) => app.input.push(ch),
            _ => {}
        }
        return Ok(false);
    }
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
        KeyCode::Char('j') | KeyCode::Down => {
            app.selected = (app.selected + 1).min(app.records.len().saturating_sub(1))
        }
        KeyCode::Char('k') | KeyCode::Up => app.selected = app.selected.saturating_sub(1),
        KeyCode::Char('n') => app.begin(Prompt::Title),
        KeyCode::Char('f') => {
            if let Some(record) = app.current() {
                app.store.update(&record.id, Change::Finish)?;
                app.message = "Spec done; Jira and handoff ready".into();
            }
        }
        KeyCode::Char('J') => {
            if let Some(record) = app.current() {
                app.begin(Prompt::Jira {
                    id: record.id.clone(),
                });
            }
        }
        KeyCode::Char('i') => {
            if let Some(record) = app.current() {
                app.begin(Prompt::Agent {
                    id: record.id.clone(),
                });
            }
        }
        KeyCode::Char('p') => {
            if let Some(record) = app.current() {
                app.begin(Prompt::Pr {
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
    app.refresh()?;
    Ok(false)
}

fn open_editor(path: &std::path::Path) -> Result<()> {
    disable_raw_mode()?;
    crossterm::execute!(stdout(), LeaveAlternateScreen)?;
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "code".into());
    let result = Command::new(editor).arg(path).status();
    crossterm::execute!(stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    if !result?.success() {
        return Err("Editor failed".into());
    }
    Ok(())
}

pub fn run(store: Store) -> Result<()> {
    enable_raw_mode()?;
    crossterm::execute!(stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let result = run_loop(&mut terminal, store);
    disable_raw_mode()?;
    crossterm::execute!(stdout(), LeaveAlternateScreen)?;
    result
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, store: Store) -> Result<()> {
    let mut app = App::new(store)?;
    loop {
        app.refresh()?;
        terminal.draw(|frame| draw(frame, &mut app))?;
        if event::poll(Duration::from_secs(1))?
            && let Event::Key(key) = event::read()?
        {
            match handle_key(&mut app, key) {
                Ok(true) => break,
                Ok(false) => {}
                Err(error) => app.message = error.to_string(),
            }
        }
    }
    Ok(())
}
