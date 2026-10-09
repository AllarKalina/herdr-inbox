//! Golden snapshots of every screen: text plus styling, at three popup sizes.
//!
//! Each scenario renders into an off-screen buffer and compares it with a committed file in
//! `tests/snapshots/ui/`. Intended changes are recorded with `scripts/regress --update-snapshots`
//! and reviewed through `git diff`.

use super::*;
use crate::store::{ScanReport, SpecSource};
use ratatui::buffer::{Buffer, Cell};
use std::path::Path;

const SIZES: [(u16, u16); 3] = [(100, 35), (60, 24), (40, 18)];

struct Fixture {
    root: PathBuf,
    app: App,
    failures: Vec<String>,
}

impl Fixture {
    /// A fixed location keeps every rendered path identical between runs and machines.
    fn empty(name: &str) -> Result<Self> {
        let root = PathBuf::from("/tmp").join(format!("herdr-inbox-golden-{name}"));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("specs"))?;
        let app = App::new(Store::new(root.join("data")))?;
        Ok(Self {
            root,
            app,
            failures: Vec::new(),
        })
    }

    /// Five specs across nested folders, one at each point of the workflow.
    fn new(name: &str) -> Result<Self> {
        let mut fixture = Self::empty(name)?;
        let specs = fixture.root.join("specs");
        for (path, title) in [
            ("designs/inbox-layout.md", "Inbox layout"),
            ("designs/settings-navigation.md", "Settings navigation"),
            (
                "missions/zeller/transaction-search.md",
                "Transaction search",
            ),
            (
                "missions/zeller/settlement-reconciliation.md",
                "Settlement reconciliation across acquirers and ledgers",
            ),
            ("quarterly-plan.md", "Quarterly plan"),
        ] {
            let file = specs.join(path);
            fs::create_dir_all(file.parent().unwrap())?;
            fs::write(
                file,
                format!(
                    "# {title}\n\n## Goal\n\nDescribe {title} well enough to build it.\n\n\
                     ## Requirements\n\n- First requirement\n- Second requirement\n"
                ),
            )?;
        }
        let store = Store::new(fixture.root.join("data"));
        let mut settings = store.settings()?;
        settings.sources.push(SpecSource::new(specs)?);
        store.save_settings(&settings)?;
        store.scan()?;
        // Random record IDs would change wrapped confirmation text between runs.
        let mut records = store.list()?;
        records.sort_by(|a, b| a.title.cmp(&b.title));
        for (index, record) in records.iter().enumerate() {
            let items = fixture.root.join("data/items");
            let old = items.join(format!("{}.json", record.id));
            let id = format!("00000000-0000-4000-8000-{:012}", index + 1);
            let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&old)?)?;
            json["id"] = id.clone().into();
            fs::write(items.join(format!("{id}.json")), serde_json::to_vec(&json)?)?;
            fs::remove_file(old)?;
        }
        let id = |title: &str| -> Result<String> {
            Ok(store
                .list()?
                .into_iter()
                .find(|record| record.title.starts_with(title))
                .ok_or("fixture spec missing")?
                .id)
        };
        store.update(&id("Settings navigation")?, Change::RefineSpec)?;
        for (title, stages) in [
            ("Transaction search", 1),
            ("Settlement reconciliation", 2),
            ("Quarterly plan", 3),
        ] {
            let id = id(title)?;
            store.update(
                &id,
                Change::Jira {
                    key: "PAY-123".into(),
                    url: Some("https://jira.example/browse/PAY-123".into()),
                },
            )?;
            if stages >= 2 {
                store.update(
                    &id,
                    Change::Implement {
                        agent: Some("implementor".into()),
                        branch: Some("feature/settlement".into()),
                    },
                )?;
            }
            if stages >= 3 {
                let url = "https://github.example/org/repo/pull/42".into();
                store.update(&id, Change::Pr { url })?;
            }
        }
        fixture.app = App::new(store)?;
        // The Inbox opens on its most recently updated spec; fixture specs tie, so pin the row.
        fixture.app.tree.focused = 0;
        fixture.app.sync_tree_selection();
        Ok(fixture)
    }

    fn press(&mut self, code: KeyCode) -> Result<()> {
        handle_key(&mut self.app, KeyEvent::new(code, KeyModifiers::NONE))?;
        self.app.refresh()
    }

    fn type_text(&mut self, text: &str) -> Result<()> {
        for ch in text.chars() {
            self.press(KeyCode::Char(ch))?;
        }
        Ok(())
    }

    /// Selects a spec in the list by the start of its title.
    fn focus(&mut self, title: &str) -> Result<()> {
        let index = self
            .app
            .records
            .iter()
            .position(|record| record.title.starts_with(title))
            .ok_or("fixture spec missing")?;
        self.app.screen = Screen::List;
        self.app.tree.focus_record(index);
        self.app.sync_tree_selection();
        self.app.refresh()
    }

    fn open(&mut self, title: &str) -> Result<()> {
        self.focus(title)?;
        self.press(KeyCode::Enter)
    }

    fn set_jira(&mut self, enabled: bool) -> Result<()> {
        let mut settings = self.app.store.settings()?;
        settings.jira = enabled;
        self.app.store.save_settings(&settings)?;
        self.app.refresh()
    }

    fn check(&mut self, name: &str) -> Result<()> {
        for (width, height) in SIZES {
            let mut terminal = Terminal::new(TestBackend::new(width, height))?;
            terminal.draw(|frame| draw::draw(frame, &mut self.app))?;
            let actual = snapshot(terminal.backend().buffer());
            if let Err(failure) = compare(&format!("{name}@{width}x{height}"), &actual) {
                self.failures.push(failure);
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Result<()> {
        let failures = std::mem::take(&mut self.failures);
        assert!(
            failures.is_empty(),
            "{} snapshot(s) differ:\n\n{}\n\nIf the change is intended, run \
             `scripts/regress --update-snapshots` and review `git diff tests/snapshots`.",
            failures.len(),
            failures.join("\n\n")
        );
        Ok(())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn style(cell: &Cell) -> String {
    let mut parts = Vec::new();
    if cell.fg != Color::Reset {
        parts.push(format!("fg={:?}", cell.fg));
    }
    if cell.bg != Color::Reset {
        parts.push(format!("bg={:?}", cell.bg));
    }
    if !cell.modifier.is_empty() {
        parts.push(format!("{:?}", cell.modifier));
    }
    parts.join(" ")
}

fn snapshot(buffer: &Buffer) -> String {
    let width = usize::from(buffer.area.width);
    let mut text = String::new();
    let mut styles = String::new();
    for (y, row) in buffer.content().chunks(width).enumerate() {
        let line: String = row.iter().map(Cell::symbol).collect();
        text.push_str(line.trim_end());
        text.push('\n');
        let mut x = 0;
        while x < width {
            let current = style(&row[x]);
            let start = x;
            while x < width && style(&row[x]) == current {
                x += 1;
            }
            if !current.is_empty() {
                styles.push_str(&format!("{y:>2} {start:>3}-{:<3} {current}\n", x - 1));
            }
        }
    }
    format!("{text}--- styles: row, columns, style ---\n{styles}")
}

fn compare(name: &str, actual: &str) -> std::result::Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let golden = root.join("tests/snapshots/ui").join(format!("{name}.snap"));
    // `scripts/regress` uses this list to find committed snapshots no scenario renders any more.
    if let Some(manifest) = std::env::var_os("HERDR_INBOX_SNAPSHOT_MANIFEST") {
        use std::io::Write;
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(manifest);
        // One write per line keeps parallel scenarios from interleaving.
        let _ = file.and_then(|mut file| file.write_all(format!("{name}.snap\n").as_bytes()));
    }
    if std::env::var_os("HERDR_INBOX_UPDATE_SNAPSHOTS").is_some() {
        fs::create_dir_all(golden.parent().unwrap()).map_err(|error| error.to_string())?;
        return fs::write(&golden, actual).map_err(|error| error.to_string());
    }
    let expected = fs::read_to_string(&golden)
        .map_err(|_| format!("{name}: no golden snapshot at {}", golden.display()))?;
    if expected == actual {
        return Ok(());
    }
    let rejected = root.join("target/ui-snapshots-actual");
    let _ = fs::create_dir_all(&rejected);
    let rejected = rejected.join(format!("{name}.snap"));
    let _ = fs::write(&rejected, actual);
    let (line, (want, got)) = expected
        .lines()
        .zip(actual.lines())
        .enumerate()
        .find(|(_, (want, got))| want != got)
        .unwrap_or((
            expected.lines().count().min(actual.lines().count()),
            ("", ""),
        ));
    Err(format!(
        "{name}: first difference at line {}\n  expected: {want}\n  actual:   {got}\n  full actual output: {}",
        line + 1,
        rejected.display()
    ))
}

#[test]
fn inbox_list() -> Result<()> {
    let mut fixture = Fixture::new("list")?;
    fixture.check("list")?;
    fixture.focus("Settlement reconciliation")?;
    fixture.check("list-long-title-selected")?;
    fixture.press(KeyCode::Char('a'))?;
    fixture.check("list-archive-confirm")?;
    fixture.press(KeyCode::Esc)?;
    fixture.check("list-archive-cancelled")?;
    fixture.app.message.clear();
    fixture.app.tree.focused = 0;
    fixture.press(KeyCode::Enter)?;
    fixture.check("list-folder-collapsed")?;
    fixture.press(KeyCode::Enter)?;
    fixture.set_jira(false)?;
    fixture.check("list-jira-off")?;
    fixture.finish()
}

#[test]
fn new_spec_flow() -> Result<()> {
    let mut fixture = Fixture::new("new-spec")?;
    fixture
        .app
        .choose_client(ChoicePurpose::NewSpec, vec![Profile::Opus, Profile::Codex]);
    fixture.check("new-spec-client-choice")?;
    fixture.press(KeyCode::Char('j'))?;
    fixture.press(KeyCode::Enter)?;
    fixture.check("new-spec-workspace-prompt")?;
    fixture
        .app
        .choose_client(ChoicePurpose::NewSpec, Vec::new());
    fixture.app.prompt = None;
    fixture.check("new-spec-no-client")?;
    fixture.finish()
}

#[test]
fn detail_at_every_stage() -> Result<()> {
    let mut fixture = Fixture::new("detail")?;
    for (title, name) in [
        ("Settings navigation", "detail-spec-active"),
        ("Inbox layout", "detail-jira-ready"),
        ("Transaction search", "detail-dev-ready"),
        ("Settlement reconciliation", "detail-pr-ready"),
        ("Quarterly plan", "detail-pr-draft"),
    ] {
        fixture.open(title)?;
        fixture.check(name)?;
    }
    fixture.open("Inbox layout")?;
    fixture.press(KeyCode::Char('j'))?;
    fixture.check("detail-locked-stage-selected")?;
    fixture.press(KeyCode::Char('k'))?;
    fixture.press(KeyCode::Enter)?;
    fixture.type_text("PAY-9")?;
    fixture.check("detail-jira-key-prompt")?;
    fixture.press(KeyCode::Enter)?;
    fixture.press(KeyCode::Enter)?;
    fixture.check("detail-jira-bound-feedback")?;
    fixture.finish()
}

#[test]
fn detail_without_jira() -> Result<()> {
    let mut fixture = Fixture::new("detail-jira-off")?;
    fixture.set_jira(false)?;
    for (title, name) in [
        ("Settings navigation", "detail-jira-off-spec-active"),
        ("Inbox layout", "detail-jira-off-dev-ready"),
        ("Quarterly plan", "detail-jira-off-pr-draft"),
    ] {
        fixture.open(title)?;
        fixture.check(name)?;
    }
    fixture.finish()
}

#[test]
fn reader_and_refine_choice() -> Result<()> {
    let mut fixture = Fixture::new("reader")?;
    fixture.open("Transaction search")?;
    fixture.press(KeyCode::Char('r'))?;
    fixture.check("reader")?;
    fixture.press(KeyCode::Esc)?;
    let id = fixture.app.current().unwrap().id.clone();
    fixture.app.choose_client(
        ChoicePurpose::Refine { id },
        vec![Profile::Opus, Profile::Codex],
    );
    fixture.check("refine-client-choice")?;
    fixture.finish()
}

#[test]
fn settings_and_archive() -> Result<()> {
    let mut fixture = Fixture::new("settings")?;
    for title in ["Inbox layout", "Quarterly plan"] {
        fixture.focus(title)?;
        fixture.press(KeyCode::Char('a'))?;
        fixture.press(KeyCode::Enter)?;
    }
    fixture.app.message.clear();
    fixture.press(KeyCode::Char('s'))?;
    fixture.check("settings-folder-selected")?;
    fixture.press(KeyCode::Char('j'))?;
    fixture.press(KeyCode::Enter)?;
    fixture.check("settings-jira-turned-off")?;
    fixture.press(KeyCode::Enter)?;
    fixture.press(KeyCode::Char('j'))?;
    fixture.check("settings-archive-selected")?;
    fixture.press(KeyCode::Enter)?;
    fixture.check("archive-list")?;
    fixture.press(KeyCode::Char('j'))?;
    fixture.press(KeyCode::Char('d'))?;
    fixture.check("archive-delete-confirm")?;
    fixture.press(KeyCode::Enter)?;
    fixture.check("archive-deleted")?;
    fixture.press(KeyCode::Char('r'))?;
    fixture.check("archive-restored-empty")?;
    fixture.finish()
}

#[test]
fn first_use_and_scan_issues() -> Result<()> {
    let mut fixture = Fixture::empty("first-use")?;
    fixture.check("settings-first-use")?;
    super::super::picker::set_test_result(Err("Could not open the macOS selector".into()));
    fixture.press(KeyCode::Enter)?;
    fixture.check("settings-picker-error")?;
    fixture.finish()?;

    let mut fixture = Fixture::new("scan-issues")?;
    let report = ScanReport {
        imported: 2,
        known: 3,
        suppressed: 1,
        dropped: 1,
        issues: vec![
            "Cannot read spec /tmp/herdr-inbox-golden-scan-issues/specs/locked.md: Permission denied"
                .into(),
            "Spec outside selected folder: /tmp/elsewhere/linked.md".into(),
        ],
    };
    super::super::scan::apply(&mut fixture.app, report);
    fixture.check("scan-issues")?;
    fixture.finish()
}
