use super::*;

struct Fixture {
    root: PathBuf,
    specs: PathBuf,
    app: App,
}

impl Fixture {
    fn new() -> Result<Self> {
        let root = std::env::temp_dir().join(format!("inbox-settings-{}", Uuid::new_v4()));
        let specs = root.join("user/specs with a deliberately long folder name");
        fs::create_dir_all(specs.join("nested"))?;
        fs::write(specs.join("nested/arbitrary.md"), "# Existing plan\nBody\n")?;
        fs::write(specs.join("notes.txt"), "Not a spec")?;
        let specs = fs::canonicalize(specs)?;
        let app = App::new(Store::new(root.join("data")))?;
        Ok(Self { root, specs, app })
    }

    fn choose(&mut self, path: PathBuf) -> Result<()> {
        super::super::picker::set_test_result(Ok(Some(path)));
        press(&mut self.app, KeyCode::Enter)?;
        Ok(())
    }

    fn connect(&mut self) -> Result<()> {
        self.choose(self.specs.clone())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn press(app: &mut App, code: KeyCode) -> Result<bool> {
    handle_key(app, KeyEvent::new(code, KeyModifiers::NONE))
}

fn render(app: &mut App, width: u16, height: u16, label: &str) -> Result<Vec<String>> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| draw::draw(frame, app))?;
    let cells: Vec<Vec<_>> = terminal
        .backend()
        .buffer()
        .content()
        .chunks(width as usize)
        .map(|row| row.to_vec())
        .collect();
    let lines: Vec<String> = cells
        .iter()
        .map(|row| row.iter().map(|cell| cell.symbol()).collect())
        .collect();
    if let Some(directory) = std::env::var_os("HERDR_INBOX_UI_SNAPSHOTS") {
        let directory = PathBuf::from(directory);
        fs::create_dir_all(&directory)?;
        let name = format!("settings-{label}-{width}x{height}");
        fs::write(directory.join(format!("{name}.txt")), lines.join("\n"))?;
        let json: Vec<Vec<_>> = cells.iter().map(|row| row.iter().map(|cell| {
            serde_json::json!({"symbol": cell.symbol(), "fg": format!("{:?}", cell.fg),
                "bg": format!("{:?}", cell.bg), "bold": cell.modifier.contains(ratatui::style::Modifier::BOLD)})
        }).collect()).collect();
        fs::write(
            directory.join(format!("{name}.json")),
            serde_json::to_vec(&json)?,
        )?;
    }
    Ok(lines)
}

fn assert_minimal(app: &mut App, label: &str) -> Result<()> {
    for (width, height) in [(40, 18), (100, 35)] {
        let lines = render(app, width, height, label)?;
        assert!(lines[1].starts_with("  Inbox / Settings"), "{}", lines[1]);
        assert!(
            lines
                .iter()
                .any(|line| line.trim_start().starts_with("› Change")),
            "{lines:?}"
        );
        let footer = lines
            .iter()
            .find(|line| line.contains("Enter change"))
            .unwrap();
        assert!(footer.starts_with("  Enter change"));
        assert!(footer.contains("Esc back"));
        let text = lines.join("\n").to_lowercase();
        for removed in [
            "your folders",
            "metadata stays",
            "recursive",
            "include:",
            "exclude:",
            "save",
            "add specs",
            "add context",
            "client:",
            "workspace:",
            "restore",
            "p path",
            "j/k move",
        ] {
            assert!(
                !text.contains(removed),
                "obsolete settings control: {removed}"
            );
        }
    }
    Ok(())
}

#[test]
fn picking_a_folder_immediately_imports_and_persists_without_save() -> Result<()> {
    let mut fixture = Fixture::new()?;
    assert_eq!(fixture.app.screen, Screen::Settings);
    assert_minimal(&mut fixture.app, "empty")?;
    fixture.connect()?;
    assert_eq!(fixture.app.screen, Screen::Settings);
    assert_eq!(fixture.app.records.len(), 1);
    let record = &fixture.app.records[0];
    assert_eq!(record.title, "Existing plan");
    assert_eq!(record.spec, "done");
    assert_eq!(record.spec_path, fixture.specs.join("nested/arbitrary.md"));
    let persisted = fixture.app.store.settings()?;
    assert_eq!(persisted.sources.len(), 1);
    assert_eq!(persisted.sources[0].path, fixture.specs);
    assert_eq!(
        fixture.app.settings.config.sources[0].id,
        persisted.sources[0].id
    );
    assert_minimal(&mut fixture.app, "selected")?;
    let wide = render(&mut fixture.app, 100, 35, "selected-path")?.join("\n");
    assert!(wide.contains("specs with a deliberately long folder name"));
    press(&mut fixture.app, KeyCode::Esc)?;
    assert_eq!(fixture.app.screen, Screen::List);
    let restarted = App::new(Store::new(fixture.root.join("data")))?;
    assert_eq!(restarted.screen, Screen::List);
    assert_eq!(restarted.records.len(), 1);
    assert_eq!(restarted.records[0].id, fixture.app.records[0].id);
    Ok(())
}

#[test]
fn changing_folder_replaces_scope_instead_of_appending_old_items() -> Result<()> {
    let mut fixture = Fixture::new()?;
    fixture.connect()?;
    let old_id = fixture.app.records[0].id.clone();
    let replacement = fixture.root.join("replacement specs");
    fs::create_dir_all(replacement.join("missions/deeper"))?;
    fs::write(replacement.join("one.md"), "# New first spec\n")?;
    fs::write(
        replacement.join("missions/deeper/two.markdown"),
        "# New nested spec\n",
    )?;
    fixture.choose(replacement.clone())?;
    assert_eq!(fixture.app.screen, Screen::Settings);
    let persisted = fixture.app.store.settings()?;
    assert_eq!(persisted.sources.len(), 1);
    assert_eq!(persisted.sources[0].path, fs::canonicalize(replacement)?);
    assert_eq!(fixture.app.records.len(), 2);
    assert!(fixture.app.records.iter().all(|record| record.id != old_id));
    assert!(fixture.specs.join("nested/arbitrary.md").is_file());
    let restarted = App::new(Store::new(fixture.root.join("data")))?;
    assert_eq!(restarted.records.len(), 2);
    assert!(restarted.records.iter().all(|record| record.id != old_id));
    Ok(())
}

#[test]
fn picking_same_folder_scans_new_files_without_resetting_identity_or_progress() -> Result<()> {
    let mut fixture = Fixture::new()?;
    fixture.connect()?;
    let source_id = fixture.app.store.settings()?.sources[0].id.clone();
    let record_id = fixture.app.records[0].id.clone();
    fixture.app.store.update(
        &record_id,
        Change::Jira {
            key: "PLAN-1".into(),
            url: None,
        },
    )?;
    fs::write(fixture.specs.join("another.md"), "# Another plan\n")?;
    fixture.connect()?;
    assert_eq!(fixture.app.records.len(), 2);
    assert_eq!(fixture.app.store.settings()?.sources[0].id, source_id);
    assert_eq!(
        fixture.app.store.get(&record_id)?.jira.key.as_deref(),
        Some("PLAN-1")
    );
    assert_eq!(fixture.app.store.get(&record_id)?.spec, "done");
    fixture.connect()?;
    assert_eq!(fixture.app.records.len(), 2);
    Ok(())
}

#[test]
fn native_picker_cancel_and_failure_preserve_existing_folder_and_records() -> Result<()> {
    let mut fixture = Fixture::new()?;
    super::super::picker::set_test_result(Ok(None));
    press(&mut fixture.app, KeyCode::Enter)?;
    assert!(fixture.app.store.settings()?.sources.is_empty());
    assert!(fixture.app.records.is_empty());
    fixture.connect()?;
    let settings_before = toml::to_string(&fixture.app.store.settings()?)?;
    let record = fixture.app.records[0].clone();
    let content = fs::read(&record.spec_path)?;
    for result in [
        Ok(None),
        Err("Could not open selector".into()),
        Ok(Some(fixture.root.join("unavailable folder"))),
    ] {
        super::super::picker::set_test_result(result);
        press(&mut fixture.app, KeyCode::Enter)?;
        assert_eq!(fixture.app.screen, Screen::Settings);
        assert_eq!(
            toml::to_string(&fixture.app.store.settings()?)?,
            settings_before
        );
        assert_eq!(fixture.app.records.len(), 1);
        assert_eq!(fixture.app.records[0].id, record.id);
        assert_eq!(fs::read(&record.spec_path)?, content);
    }
    for (width, height) in [(40, 18), (100, 35)] {
        let lines = render(&mut fixture.app, width, height, "picker-error")?;
        assert!(lines.iter().any(|line| line.contains("Change")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Enter change · Esc back"))
        );
        assert!(!fixture.app.message.is_empty());
    }
    Ok(())
}

#[test]
fn removed_setup_shortcuts_cannot_edit_or_scan_settings() -> Result<()> {
    let mut fixture = Fixture::new()?;
    fixture.connect()?;
    let before = toml::to_string(&fixture.app.store.settings()?)?;
    fs::write(fixture.specs.join("unscanned.md"), "# Not scanned yet\n")?;
    for key in ['s', 'a', 'c', 'p', 'd', 'm', 'j', 'k'] {
        press(&mut fixture.app, KeyCode::Char(key))?;
        assert!(fixture.app.prompt.is_none());
        assert_eq!(fixture.app.screen, Screen::Settings);
        assert_eq!(fixture.app.records.len(), 1);
        assert_eq!(toml::to_string(&fixture.app.store.settings()?)?, before);
    }
    Ok(())
}

#[test]
fn clicking_change_opens_the_selector_and_background_clicks_do_not() -> Result<()> {
    let mut fixture = Fixture::new()?;
    render(&mut fixture.app, 40, 18, "mouse-change")?;
    let change = fixture.app.settings.change_area;
    assert!(!change.is_empty());
    super::super::picker::set_test_result(Ok(Some(fixture.specs.clone())));
    handle_mouse(
        &mut fixture.app,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        },
        18,
    )?;
    assert!(fixture.app.store.settings()?.sources.is_empty());
    handle_mouse(
        &mut fixture.app,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: change.x + 3,
            row: change.y,
            modifiers: KeyModifiers::NONE,
        },
        18,
    )?;
    assert_eq!(fixture.app.store.settings()?.sources[0].path, fixture.specs);
    assert_eq!(fixture.app.records.len(), 1);
    assert_eq!(fixture.app.screen, Screen::Settings);
    Ok(())
}
