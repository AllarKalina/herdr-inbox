use super::*;
use crate::store::SpecSource;

struct Fixture {
    root: PathBuf,
    specs: PathBuf,
    context: PathBuf,
    app: App,
}

impl Fixture {
    fn new() -> Result<Self> {
        let root = std::env::temp_dir().join(format!("inbox-settings-{}", Uuid::new_v4()));
        let specs = root.join("user/odd structure/nested");
        fs::create_dir_all(&specs)?;
        fs::write(
            specs.join("an arbitrary filename.md"),
            "# Existing plan\nBody\n",
        )?;
        let specs = specs.parent().unwrap().to_path_buf();
        let context = root.join("notes.txt");
        fs::write(&context, "My context")?;
        let app = App::new(Store::new(root.join("data")))?;
        Ok(Self {
            root,
            specs,
            context,
            app,
        })
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

fn type_text(app: &mut App, text: &str) -> Result<()> {
    for character in text.chars() {
        press(app, KeyCode::Char(character))?;
    }
    Ok(())
}

fn render(app: &mut App, width: u16, height: u16) -> Result<String> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| draw::draw(frame, app))?;
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .all(|cell| cell.bg == Color::Reset || cell.bg == Color::Cyan)
    );
    let text = terminal
        .backend()
        .buffer()
        .content()
        .chunks(width as usize)
        .map(|cells| cells.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    if let Some(directory) = std::env::var_os("HERDR_INBOX_UI_SNAPSHOTS") {
        fs::create_dir_all(&directory)?;
        fs::write(
            PathBuf::from(directory).join(format!("{:?}-{width}x{height}.txt", app.screen)),
            &text,
        )?;
    }
    Ok(text)
}

#[test]
fn keyboard_setup_imports_user_folders_and_persists_settings() -> Result<()> {
    let mut fixture = Fixture::new()?;
    assert_eq!(fixture.app.screen, Screen::Settings);
    for (width, height) in [(40, 18), (60, 24), (100, 35)] {
        let text = render(&mut fixture.app, width, height)?;
        assert!(text.contains("Connect your specs"));
        assert!(text.contains("Add specs folder"));
        assert!(text.contains("s save"));
    }
    press(&mut fixture.app, KeyCode::Char('a'))?;
    type_text(&mut fixture.app, &fixture.specs.display().to_string())?;
    press(&mut fixture.app, KeyCode::Enter)?;
    press(&mut fixture.app, KeyCode::Char('c'))?;
    type_text(&mut fixture.app, &fixture.context.display().to_string())?;
    press(&mut fixture.app, KeyCode::Enter)?;
    for (width, height) in [(40, 18), (100, 35)] {
        let text = render(&mut fixture.app, width, height)?;
        assert!(text.contains("Specs:"));
        assert!(text.contains("Left/Right scroll long paths"));
    }
    press(&mut fixture.app, KeyCode::Char('s'))?;
    assert_eq!(fixture.app.screen, Screen::List);
    assert!(fixture.app.message.contains('1'));
    assert_eq!(fixture.app.records.len(), 1);
    let record = fixture.app.records[0].clone();
    assert_eq!(record.title, "Existing plan");
    assert_eq!(record.spec, "done");
    assert_eq!(record.jira.status, "ready");
    assert_eq!(record.implementation_stage(), "locked");
    assert_eq!(record.pr_stage(), "locked");
    let persisted = fixture.app.store.settings()?;
    assert_eq!(persisted.sources[0].path, fixture.specs);
    assert_eq!(persisted.context_paths, vec![fixture.context.clone()]);
    fixture.app.store.update(
        &record.id,
        Change::Jira {
            key: "PLAN-1".into(),
            url: None,
        },
    )?;
    fs::write(fixture.specs.join("another.markdown"), "# Another plan\n")?;
    press(&mut fixture.app, KeyCode::Char('s'))?;
    press(&mut fixture.app, KeyCode::Char('s'))?;
    assert_eq!(fixture.app.records.len(), 2);
    assert_eq!(
        fixture.app.store.get(&record.id)?.jira.key.as_deref(),
        Some("PLAN-1")
    );
    let restarted = App::new(Store::new(fixture.root.join("data")))?;
    assert_eq!(restarted.screen, Screen::List);
    assert_eq!(restarted.records.len(), 2);
    Ok(())
}

#[test]
fn invalid_folder_remains_editable_and_cancel_preserves_saved_settings() -> Result<()> {
    let mut fixture = Fixture::new()?;
    press(&mut fixture.app, KeyCode::Char('a'))?;
    type_text(&mut fixture.app, "/missing/never-existing-inbox-path")?;
    press(&mut fixture.app, KeyCode::Enter)?;
    let text = render(&mut fixture.app, 60, 24)?;
    assert!(text.contains("missing") || text.contains("exist"));
    assert!(text.contains("█"));
    assert!(fixture.app.store.settings()?.sources.is_empty());
    press(&mut fixture.app, KeyCode::Esc)?;
    press(&mut fixture.app, KeyCode::Esc)?;
    assert_eq!(fixture.app.screen, Screen::List);
    assert!(fixture.app.store.settings()?.sources.is_empty());
    press(&mut fixture.app, KeyCode::Char('s'))?;
    assert_eq!(fixture.app.screen, Screen::Settings);
    Ok(())
}

#[test]
fn trash_picker_restores_import_uuid_without_modifying_user_spec() -> Result<()> {
    let mut fixture = Fixture::new()?;
    let mut settings = fixture.app.store.settings()?;
    settings
        .sources
        .push(SpecSource::new(fixture.specs.clone())?);
    fixture.app.store.save_settings(&settings)?;
    fixture.app.store.scan()?;
    fixture.app.screen = Screen::List;
    fixture.app.refresh()?;
    let record = fixture.app.current().unwrap().clone();
    let content = fs::read(&record.spec_path)?;
    press(&mut fixture.app, KeyCode::Char('a'))?;
    press(&mut fixture.app, KeyCode::Enter)?;
    press(&mut fixture.app, KeyCode::Char('s'))?;
    press(&mut fixture.app, KeyCode::Char('s'))?;
    assert!(fixture.app.records.is_empty());
    assert_eq!(fs::read(&record.spec_path)?, content);
    press(&mut fixture.app, KeyCode::Char('s'))?;
    for _ in 0..20 {
        press(&mut fixture.app, KeyCode::Down)?;
    }
    press(&mut fixture.app, KeyCode::Enter)?;
    assert_eq!(fixture.app.screen, Screen::Trash);
    for (width, height) in [(40, 18), (100, 35)] {
        let text = render(&mut fixture.app, width, height)?;
        assert!(text.contains("Archived items"));
        assert!(text.contains("Existing plan"));
        assert!(text.contains("Enter restore"));
    }
    press(&mut fixture.app, KeyCode::Enter)?;
    assert_eq!(fixture.app.screen, Screen::Settings);
    assert_eq!(fixture.app.current().unwrap().id, record.id);
    assert_eq!(fs::read(&record.spec_path)?, content);
    Ok(())
}

#[test]
fn detail_relinks_missing_file_and_new_session_prompts_explicit_destination() -> Result<()> {
    let mut fixture = Fixture::new()?;
    let mut settings = fixture.app.store.settings()?;
    settings
        .sources
        .push(SpecSource::new(fixture.specs.clone())?);
    settings.workspace = "my-workspace".into();
    settings.preferred_client = Some("codex".into());
    fixture.app.store.save_settings(&settings)?;
    fixture.app.store.scan()?;
    fixture.app.screen = Screen::List;
    fixture.app.refresh()?;
    let record = fixture.app.current().unwrap().clone();
    let renamed = fixture.specs.join("moved plan.md");
    fs::rename(&record.spec_path, &renamed)?;
    assert!(render(&mut fixture.app, 100, 24)?.contains("unavailable"));
    press(&mut fixture.app, KeyCode::Enter)?;
    press(&mut fixture.app, KeyCode::Char('L'))?;
    type_text(&mut fixture.app, &renamed.display().to_string())?;
    press(&mut fixture.app, KeyCode::Enter)?;
    assert_eq!(
        fs::canonicalize(fixture.app.store.get(&record.id)?.spec_path)?,
        fs::canonicalize(renamed)?
    );
    fixture.app.screen = Screen::List;
    fixture
        .app
        .choose_client(ChoicePurpose::NewSpec, vec![Profile::Opus, Profile::Codex]);
    assert_eq!(fixture.app.choice_selected, Some(1));
    press(&mut fixture.app, KeyCode::Enter)?;
    assert_eq!(fixture.app.input, "my-workspace");
    press(&mut fixture.app, KeyCode::Enter)?;
    assert!(matches!(
        fixture.app.prompt,
        Some(Prompt::LaunchRepo { .. })
    ));
    press(&mut fixture.app, KeyCode::Enter)?;
    assert!(matches!(
        fixture.app.prompt,
        Some(Prompt::LaunchSpec { .. })
    ));
    type_text(
        &mut fixture.app,
        &fixture
            .specs
            .join("my future spec.md")
            .display()
            .to_string(),
    )?;
    press(&mut fixture.app, KeyCode::Enter)?;
    assert!(
        matches!(&fixture.app.prompt, Some(Prompt::LaunchTopic { spec: Some(path), .. }) if path.ends_with("my future spec.md"))
    );
    // Cancelling here never touches Herdr or creates a spec file.
    press(&mut fixture.app, KeyCode::Esc)?;
    assert!(!fixture.app.should_exit);
    assert_eq!(fixture.app.store.list()?.len(), 1);
    Ok(())
}

#[test]
fn scan_diagnostics_and_settle_confirmation_render_and_keep_state_safe() -> Result<()> {
    let mut fixture = Fixture::new()?;
    let mut settings = fixture.app.store.settings()?;
    settings
        .sources
        .push(SpecSource::new(fixture.specs.clone())?);
    fixture.app.store.save_settings(&settings)?;
    fixture.app.store.scan()?;
    fixture.app.screen = Screen::List;
    fixture.app.refresh()?;
    let record = fixture.app.current().unwrap().clone();
    fixture.app.store.update(
        &record.id,
        Change::Launch(
            Box::new(crate::store::Launch {
                status: "prompt_sent".into(),
                harness: "codex".into(),
                workspace: "test".into(),
                workspace_id: None,
                tab_id: None,
                pane_id: None,
                agent: None,
                model: "test".into(),
                effort: "high".into(),
                prompt: "Test only; no live session".into(),
                error: None,
            }),
            record.spec_path.clone(),
        ),
    )?;
    fixture.app.refresh()?;
    press(&mut fixture.app, KeyCode::Enter)?;
    press(&mut fixture.app, KeyCode::Char('x'))?;
    for (width, height) in [(40, 18), (100, 35)] {
        let text = render(&mut fixture.app, width, height)?;
        assert!(text.contains("Session ended?"));
        assert!(text.contains("Enter settle"));
        assert!(text.contains("Esc cancel"));
    }
    press(&mut fixture.app, KeyCode::Char('x'))?;
    assert!(fixture.app.store.get(&record.id)?.active_spec_session());
    press(&mut fixture.app, KeyCode::Esc)?;
    assert!(fixture.app.store.get(&record.id)?.active_spec_session());
    press(&mut fixture.app, KeyCode::Char('x'))?;
    press(&mut fixture.app, KeyCode::Enter)?;
    assert!(!fixture.app.store.get(&record.id)?.active_spec_session());
    assert_eq!(fixture.app.store.get(&record.id)?.spec, record.spec);
    press(&mut fixture.app, KeyCode::Esc)?;
    fs::rename(&fixture.specs, fixture.root.join("elsewhere"))?;
    press(&mut fixture.app, KeyCode::Char('s'))?;
    press(&mut fixture.app, KeyCode::Char('s'))?;
    assert_eq!(fixture.app.screen, Screen::ScanResult);
    for (width, height) in [(40, 18), (100, 35)] {
        let text = render(&mut fixture.app, width, height)?;
        assert!(text.contains("Scan results"));
        assert!(text.contains("Source unavailable"));
        assert!(text.contains("s settings"));
    }
    press(&mut fixture.app, KeyCode::Enter)?;
    assert_eq!(fixture.app.records.len(), 1);
    Ok(())
}

#[test]
fn source_relocation_requires_confirmation_and_preserves_identity() -> Result<()> {
    let mut fixture = Fixture::new()?;
    let mut settings = fixture.app.store.settings()?;
    settings
        .sources
        .push(SpecSource::new(fixture.specs.clone())?);
    let source_id = settings.sources[0].id.clone();
    fixture.app.store.save_settings(&settings)?;
    fixture.app.store.scan()?;
    fixture.app.screen = Screen::List;
    fixture.app.refresh()?;
    let record = fixture.app.current().unwrap().clone();
    let destination = fixture.root.join("my moved specs");
    fs::rename(&fixture.specs, &destination)?;
    press(&mut fixture.app, KeyCode::Char('s'))?;
    press(&mut fixture.app, KeyCode::Char('m'))?;
    type_text(&mut fixture.app, &destination.display().to_string())?;
    press(&mut fixture.app, KeyCode::Enter)?;
    assert_eq!(fixture.app.store.settings()?.sources[0].path, fixture.specs);
    press(&mut fixture.app, KeyCode::Esc)?;
    assert_eq!(fixture.app.store.settings()?.sources[0].path, fixture.specs);
    press(&mut fixture.app, KeyCode::Char('m'))?;
    type_text(&mut fixture.app, &destination.display().to_string())?;
    press(&mut fixture.app, KeyCode::Enter)?;
    press(&mut fixture.app, KeyCode::Enter)?;
    let relocated = fixture.app.store.get(&record.id)?;
    assert_eq!(relocated.source_id.as_deref(), Some(source_id.as_str()));
    assert_eq!(relocated.spec, record.spec);
    assert!(relocated.spec_path.is_file());
    assert_eq!(fixture.app.store.list()?.len(), 1);
    assert_eq!(fixture.app.store.settings()?.sources[0].id, source_id);
    Ok(())
}

#[test]
fn trash_back_returns_to_unsaved_settings_draft() -> Result<()> {
    let mut fixture = Fixture::new()?;
    press(&mut fixture.app, KeyCode::Char('a'))?;
    type_text(&mut fixture.app, &fixture.specs.display().to_string())?;
    press(&mut fixture.app, KeyCode::Enter)?;
    super::super::trash::open(&mut fixture.app)?;
    press(&mut fixture.app, KeyCode::Esc)?;
    assert_eq!(fixture.app.screen, Screen::Settings);
    assert_eq!(fixture.app.settings.draft.sources.len(), 1);
    assert!(fixture.app.store.settings()?.sources.is_empty());
    Ok(())
}
