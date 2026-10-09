//! Golden snapshots of every screen: text plus styling, at three popup sizes.
//!
//! Each scenario renders into an off-screen buffer and compares it with a committed file in
//! `tests/snapshots/ui/`. Intended changes are recorded with `scripts/regress --update-snapshots`
//! and reviewed through `git diff`.

use super::*;
use crate::launch::Profile;
use crate::store::{Launch, LaunchStatus, ScanReport, SpecSource};
use ratatui::buffer::{Buffer, Cell};
use std::path::Path;

const SIZES: [(u16, u16); 3] = [(100, 35), (60, 24), (40, 18)];

pub(super) struct Fixture {
    pub(super) root: PathBuf,
    pub(super) app: App,
    failures: Vec<String>,
}

impl Fixture {
    /// A fixed location keeps every rendered path identical between runs and machines.
    pub(super) fn empty(name: &str) -> Result<Self> {
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
    pub(super) fn new(name: &str) -> Result<Self> {
        let mut fixture = Self::empty(name)?;
        let specs = fixture.root.join("specs");
        for (path, title) in [
            ("designs/inbox-layout.md", "Inbox layout"),
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
        fs::write(specs.join("designs/preview.html"), "<h1>Preview</h1>\n")?;
        let store = Store::new(fixture.root.join("data"));
        let mut settings = store.settings()?;
        let mut source = SpecSource::new(specs.clone())?;
        source.include.push("**/*.html".into());
        settings.sources.push(source);
        store.save_settings(&settings)?;
        // A spec still being written: Jira waits, and its session is live.
        let started = store.start(
            "Settings navigation",
            Some(specs.join("designs/settings-navigation.md")),
        )?;
        let launch = Launch {
            status: LaunchStatus::PromptSent,
            harness: "codex".into(),
            workspace: "ai-boiler-room".into(),
            workspace_id: Some("w1".into()),
            tab_id: Some("w1:t1".into()),
            pane_id: Some("w1:p1".into()),
            agent: Some("spec_00000000".into()),
            model: "gpt-6.1-sol".into(),
            effort: "high".into(),
            prompt: "Fixture prompt".into(),
            error: None,
        };
        let launched = Change::Launch(Box::new(launch), started.spec_path.clone());
        store.update(&started.id, launched)?;
        store.scan()?;
        let id = |title: &str| -> Result<String> {
            Ok(store
                .list()?
                .into_iter()
                .find(|record| record.title.starts_with(title))
                .ok_or("fixture spec missing")?
                .id)
        };
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
        // Random IDs and wall-clock timestamps would make text and ordering differ between runs.
        let mut records = store.list()?;
        records.sort_by(|a, b| a.title.cmp(&b.title));
        for (index, record) in records.iter().enumerate() {
            let items = fixture.root.join("data/items");
            let old = items.join(format!("{}.json", record.id));
            let id = format!("00000000-0000-4000-8000-{:012}", index + 1);
            let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&old)?)?;
            json["id"] = id.clone().into();
            // Equal timestamps would leave the record order to chance.
            json["created_at"] = (1_700_000_000 + index).into();
            json["updated_at"] = (1_700_000_000 + index).into();
            fs::write(items.join(format!("{id}.json")), serde_json::to_vec(&json)?)?;
            fs::remove_file(old)?;
        }
        fixture.app = App::new(store)?;
        // The Inbox opens on its most recently updated spec; fixture specs tie, so pin the row.
        fixture.app.list.tree.focused = 0;
        Ok(fixture)
    }

    pub(super) fn press(&mut self, code: KeyCode) -> Result<()> {
        handle_key(&mut self.app, KeyEvent::new(code, KeyModifiers::NONE))?;
        self.app.refresh()
    }

    pub(super) fn type_text(&mut self, text: &str) -> Result<()> {
        for ch in text.chars() {
            self.press(KeyCode::Char(ch))?;
        }
        Ok(())
    }

    /// Selects a spec in the list by the start of its title.
    pub(super) fn focus(&mut self, title: &str) -> Result<()> {
        let index = self
            .app
            .records
            .iter()
            .position(|record| record.title.starts_with(title))
            .ok_or("fixture spec missing")?;
        self.app.screen = Screen::List;
        self.app.list.tree.focus_record(index);
        self.app.refresh()
    }

    pub(super) fn open(&mut self, title: &str) -> Result<()> {
        self.focus(title)?;
        self.press(KeyCode::Enter)
    }

    pub(super) fn set_jira(&mut self, enabled: bool) -> Result<()> {
        let mut settings = self.app.store.settings()?;
        settings.jira = enabled;
        self.app.store.save_settings(&settings)?;
        self.app.refresh()
    }

    pub(super) fn check(&mut self, name: &str) -> Result<()> {
        for (width, height) in SIZES {
            let mut terminal = Terminal::new(TestBackend::new(width, height))?;
            terminal.draw(|frame| draw(frame, &mut self.app))?;
            let actual = snapshot(terminal.backend().buffer());
            if let Err(failure) = compare(&format!("{name}@{width}x{height}"), &actual) {
                self.failures.push(failure);
            }
        }
        Ok(())
    }

    pub(super) fn finish(mut self) -> Result<()> {
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
    let (mut want, mut got) = (expected.lines(), actual.lines());
    let mut line = 1;
    let (want, got) = loop {
        match (want.next(), got.next()) {
            (Some(want), Some(got)) if want == got => line += 1,
            (want, got) => break (want.unwrap_or("<end>"), got.unwrap_or("<end>")),
        }
    };
    Err(format!(
        "{name}: first difference at line {}\n  expected: {want}\n  actual:   {got}\n  full actual output: {}",
        line,
        rejected.display()
    ))
}

mod detail;
mod flows;
mod list;
