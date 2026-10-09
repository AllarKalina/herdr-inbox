//! What the UI tests share: a fixture, input helpers that go through the real event
//! dispatch, and off-screen rendering.

use super::*;
use ratatui::buffer::Cell;

pub(super) fn configured_store(root: PathBuf) -> Result<Store> {
    let store = Store::new(root.clone());
    let specs = root.join("specs");
    fs::create_dir_all(&specs)?;
    let mut settings = store.settings()?;
    settings.sources.push(crate::store::SpecSource::new(specs)?);
    store.save_settings(&settings)?;
    Ok(store)
}

/// One spec, "Payment retries", advanced through `stage` workflow steps (0 = new, 1 = spec
/// done, 2 = Jira linked, 3 = implementation started, 4 = draft PR) and opened in detail.
pub(super) struct Fixture {
    pub app: App,
    pub id: String,
    /// The data directory; the specs folder is `root/specs`.
    pub root: PathBuf,
}

impl Fixture {
    pub(super) fn new(stage: usize) -> Result<Self> {
        let root = std::env::temp_dir().join(format!("herdr-inbox-ui-{}", Uuid::new_v4()));
        let store = configured_store(root.clone())?;
        let record = store.start("Payment retries", None)?;
        if stage >= 1 {
            store.update(
                &record.id,
                Change::Finish {
                    title: None,
                    spec: None,
                },
            )?;
        }
        if stage >= 2 {
            store.update(
                &record.id,
                Change::Jira {
                    key: "PAY-123".into(),
                    url: Some("https://jira.example/browse/PAY-123".into()),
                },
            )?;
        }
        if stage >= 3 {
            store.update(
                &record.id,
                Change::Implement {
                    agent: Some("implementor".into()),
                    branch: Some("feature/payment-retries".into()),
                },
            )?;
        }
        if stage >= 4 {
            store.update(
                &record.id,
                Change::Pr {
                    url: "https://github.example/org/repo/pull/42".into(),
                },
            )?;
        }
        let mut fixture = Self {
            app: App::new(store)?,
            id: record.id,
            root,
        };
        fixture.open()?;
        Ok(fixture)
    }
}

impl Fixture {
    /// Opens the item selected in the list.
    pub(super) fn open(&mut self) -> Result<()> {
        press(&mut self.app, KeyCode::Enter)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub(super) fn press(app: &mut App, code: KeyCode) -> Result<()> {
    handle_key(app, KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

pub(super) fn type_text(app: &mut App, text: &str) -> Result<()> {
    text.chars()
        .try_for_each(|ch| press(app, KeyCode::Char(ch)))
}

pub(super) fn mouse(app: &mut App, kind: MouseEventKind, column: u16, row: u16) -> Result<()> {
    handle_mouse(
        app,
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        },
    )
}

pub(super) fn click(app: &mut App, column: u16, row: u16) -> Result<()> {
    mouse(app, MouseEventKind::Down(MouseButton::Left), column, row)
}

/// The row on which a milestone's label and node are drawn.
pub(super) fn node(app: &App, stage: Milestone) -> Rect {
    app.detail
        .milestone_hitboxes
        .iter()
        .find(|(milestone, _)| *milestone == stage)
        .expect("every stage remains visible")
        .1
}

pub(super) fn render(app: &mut App, width: u16, height: u16) -> Result<Terminal<TestBackend>> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| draw(frame, app))?;
    Ok(terminal)
}

/// Renders the current screen and returns its cells row by row, with their styling.
pub(super) fn cells(app: &mut App, width: u16, height: u16) -> Result<Vec<Vec<Cell>>> {
    Ok(render(app, width, height)?
        .backend()
        .buffer()
        .content()
        .chunks(usize::from(width).max(1))
        .map(<[Cell]>::to_vec)
        .collect())
}

pub(super) fn text(cells: &[Vec<Cell>]) -> Vec<String> {
    cells
        .iter()
        .map(|row| row.iter().map(Cell::symbol).collect())
        .collect()
}

/// Renders the current screen and returns its text, one string per row.
pub(super) fn lines(app: &mut App, width: u16, height: u16) -> Result<Vec<String>> {
    Ok(text(&cells(app, width, height)?))
}

/// Selects one of the current milestone's actions and runs it.
pub(super) fn run_action(app: &mut App, action: DetailAction) -> Result<()> {
    app.detail.action = app
        .actions()
        .iter()
        .position(|candidate| *candidate == action)
        .expect("the milestone offers this action");
    press(app, KeyCode::Enter)
}

/// Selects an item's row in the list.
pub(super) fn focus(app: &mut App, id: &str) {
    let index = app
        .records
        .iter()
        .position(|record| record.id == id)
        .expect("item is in the Inbox");
    app.list.tree.focus_record(index);
}
