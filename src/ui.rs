//! The terminal UI. `App` holds what every screen shares; each screen keeps its own state
//! in its module under `screens/` and exposes the same four functions: `draw`,
//! `handle_key`, `handle_mouse` and `crumbs`. `Screen` dispatches to them, so adding a
//! screen means adding a module, a variant, and one arm per function.

mod chrome;
mod milestone;
mod modal;
mod picker;
mod screens;
mod state;
#[cfg(test)]
mod tests;
mod text;
mod theme;
mod tree;

use milestone::Milestone;
use screens::{archive, detail, list, reader, scan, settings};
use state::{ChoicePurpose, DetailAction, MilestoneFeedback, Notice, Prompt};

use crate::settings::Settings;
use crate::store::{Record, Result, Store};
use crossterm::event::{self, Event, KeyEvent, KeyEventKind, MouseEvent};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{self, stdout};
use std::time::Duration;

struct App {
    store: Store,
    /// This computer's configuration, reread on every refresh.
    config: Settings,
    /// Inbox items: records whose spec file exists inside a selected folder.
    records: Vec<Record>,
    screen: Screen,
    list: list::View,
    detail: detail::View,
    reader: reader::View,
    settings: settings::View,
    archive: archive::View,
    scan: scan::View,
    modal: modal::Modal,
    notice: Notice,
    should_exit: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    List,
    Detail,
    Reader,
    Settings,
    Archive,
    Scan,
}

impl Screen {
    fn draw(self, frame: &mut ratatui::Frame, app: &mut App) {
        match self {
            Self::List => list::draw(frame, app),
            Self::Detail => detail::draw(frame, app),
            Self::Reader => reader::draw(frame, app),
            Self::Settings => settings::draw(frame, app),
            Self::Archive => archive::draw(frame, app),
            Self::Scan => scan::draw(frame, app),
        }
    }

    /// Returns true when the Inbox should close.
    fn handle_key(self, app: &mut App, key: KeyEvent) -> Result<bool> {
        match self {
            Self::List => list::handle_key(app, key),
            Self::Detail => detail::handle_key(app, key),
            Self::Reader => reader::handle_key(app, key),
            Self::Settings => settings::handle_key(app, key),
            Self::Archive => archive::handle_key(app, key),
            Self::Scan => scan::handle_key(app, key),
        }
    }

    fn handle_mouse(self, app: &mut App, mouse: MouseEvent) -> Result<()> {
        match self {
            Self::List => list::handle_mouse(app, mouse),
            Self::Detail => detail::handle_mouse(app, mouse),
            Self::Reader => reader::handle_mouse(app, mouse),
            Self::Settings => settings::handle_mouse(app, mouse),
            Self::Archive => archive::handle_mouse(app, mouse),
            Self::Scan => Ok(()),
        }
    }

    /// The breadcrumb segments after the anchored `Inbox`.
    fn crumbs(self, app: &App) -> Vec<String> {
        match self {
            Self::List => Vec::new(),
            Self::Detail => detail::crumbs(app),
            Self::Reader => reader::crumbs(app),
            Self::Settings => settings::crumbs(),
            Self::Archive => archive::crumbs(),
            Self::Scan => scan::crumbs(),
        }
    }
}

impl App {
    fn new(store: Store) -> Result<Self> {
        let config = store.settings()?;
        let report = store.scan()?;
        let screen = if config.sources.is_empty() {
            Screen::Settings
        } else if !report.issues.is_empty() {
            Screen::Scan
        } else {
            Screen::List
        };
        let mut app = Self {
            store,
            config,
            records: Vec::new(),
            screen,
            list: list::View::default(),
            detail: detail::View::default(),
            reader: reader::View::default(),
            settings: settings::View::default(),
            archive: archive::View::default(),
            scan: scan::View::new(report),
            modal: modal::Modal::default(),
            notice: Notice::default(),
            should_exit: false,
        };
        app.refresh()?;
        Ok(app)
    }

    /// Rereads settings and records from disk and reconciles every view with them. Runs
    /// once a second and after every action, so CLI and agent updates show up unprompted.
    fn refresh(&mut self) -> Result<()> {
        let was_next = self.next_milestone();
        self.config = self.store.settings()?;
        self.records = self
            .store
            .list()?
            .into_iter()
            .filter(|record| tree::includes(record, &self.config.sources))
            .collect();
        self.list.tree.rebuild(&self.records, &self.config.sources);
        self.modal.drop_orphans(&self.records);
        if matches!(self.screen, Screen::Settings | Screen::Archive) {
            archive::reload(self)?;
        }
        if self.config.sources.is_empty()
            && !matches!(self.screen, Screen::Settings | Screen::Archive)
        {
            // Without a folder there is nothing to show but the place to choose one.
            self.modal.close();
            settings::open(self);
        }
        if matches!(self.screen, Screen::Detail | Screen::Reader) {
            detail::reconcile(self, was_next);
        }
        Ok(())
    }

    /// The item the user is looking at: the open one, or the selected row in the list.
    fn current(&self) -> Option<&Record> {
        match self.screen {
            Screen::Detail | Screen::Reader => self.record(self.detail.record.as_deref()?),
            _ => self
                .list
                .tree
                .selected_record()
                .and_then(|index| self.records.get(index)),
        }
    }

    fn record(&self, id: &str) -> Option<&Record> {
        self.records.iter().find(|record| record.id == id)
    }

    /// Whether the Jira stage is part of this computer's workflow.
    fn jira(&self) -> bool {
        self.config.jira
    }

    fn next_milestone(&self) -> Option<Milestone> {
        self.current()
            .map(|record| Milestone::next(record, self.jira()))
    }

    fn crumbs(&self) -> Vec<String> {
        modal::crumbs(self).unwrap_or_else(|| self.screen.crumbs(self))
    }
}

fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    chrome::draw_header(frame, &app.crumbs());
    app.screen.draw(frame, app);
}

/// Applies one input event and returns true when the Inbox should close. A failed action
/// becomes the notice; nothing a user does can end the session with an error.
fn dispatch(app: &mut App, event: Event) -> bool {
    let outcome = match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => handle_key(app, key),
        Event::Mouse(mouse) if !app.modal.is_open() => {
            app.screen.handle_mouse(app, mouse).map(|()| false)
        }
        _ => Ok(false),
    };
    match outcome {
        Ok(close) => close || app.should_exit,
        Err(error) => {
            app.notice.error(error.to_string());
            false
        }
    }
}

fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    // An open prompt or client choice owns the keyboard until it is answered or dismissed.
    let close = match modal::handle_key(app, key) {
        Some(result) => result?,
        None => app.screen.handle_key(app, key)?,
    };
    app.refresh()?;
    Ok(close)
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    crossterm::execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen)
}

pub fn run(store: Store) -> Result<()> {
    // A panic must not strand the popup in raw mode on the alternate screen.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        default_hook(info);
    }));
    enable_raw_mode()?;
    crossterm::execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let result = Terminal::new(CrosstermBackend::new(stdout()))
        .map_err(Into::into)
        .and_then(|mut terminal| run_loop(&mut terminal, store));
    restore_terminal()?;
    result
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, store: Store) -> Result<()> {
    let mut app = App::new(store)?;
    loop {
        // Another process may be mid-write; a failed refresh is reported, not fatal.
        if let Err(error) = app.refresh() {
            app.notice.error(error.to_string());
        }
        terminal.draw(|frame| draw(frame, &mut app))?;
        if event::poll(Duration::from_secs(1))? && dispatch(&mut app, event::read()?) {
            return Ok(());
        }
    }
}
