//! Settings: choosing the folder, the Jira switch, and the archive list.

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
        crate::ui::settings::picker::set_test_result(Ok(Some(path)));
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

use super::support::lines as render;

#[test]
fn picking_a_folder_immediately_imports_and_persists_without_save() -> Result<()> {
    let mut fixture = Fixture::new()?;
    assert_eq!(fixture.app.screen, Screen::Settings);
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
    assert_eq!(fixture.app.config.sources[0].id, persisted.sources[0].id);
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
    crate::ui::settings::picker::set_test_result(Ok(None));
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
        crate::ui::settings::picker::set_test_result(result);
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
    assert!(!fixture.app.notice.is_empty());
    Ok(())
}

#[test]
fn clicking_change_opens_the_selector_and_background_clicks_do_not() -> Result<()> {
    let mut fixture = Fixture::new()?;
    render(&mut fixture.app, 40, 18)?;
    let change = fixture.app.settings.rows_area;
    assert_eq!(change.height, 3);
    crate::ui::settings::picker::set_test_result(Ok(Some(fixture.specs.clone())));
    handle_mouse(
        &mut fixture.app,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        },
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
    )?;
    assert_eq!(fixture.app.store.settings()?.sources[0].path, fixture.specs);
    assert_eq!(fixture.app.records.len(), 1);
    assert_eq!(fixture.app.screen, Screen::Settings);
    Ok(())
}

#[test]
fn picking_the_same_folder_again_keeps_its_cli_configured_filters() -> Result<()> {
    let mut fixture = Fixture::new()?;
    fixture.connect()?;
    fs::write(fixture.specs.join("page.html"), "<h1>Page</h1>")?;
    let mut settings = fixture.app.store.settings()?;
    settings.sources[0].include = vec!["**/*.md".into(), "**/*.html".into()];
    settings.sources[0].exclude = vec!["drafts/**".into()];
    settings.sources[0].recursive = false;
    fixture.app.store.save_settings(&settings)?;
    fixture.app.refresh()?;

    fixture.connect()?;
    let kept = fixture.app.store.settings()?;
    assert_eq!(kept.sources.len(), 1);
    assert_eq!(kept.sources[0].id, settings.sources[0].id);
    assert_eq!(kept.sources[0].include, settings.sources[0].include);
    assert_eq!(kept.sources[0].exclude, settings.sources[0].exclude);
    assert!(!kept.sources[0].recursive);
    assert!(
        fixture
            .app
            .records
            .iter()
            .any(|record| record.spec_path.ends_with("page.html"))
    );
    Ok(())
}

fn open_settings_row(app: &mut App, row: usize) -> Result<()> {
    press(app, KeyCode::Char('s'))?;
    assert_eq!(app.screen, Screen::Settings);
    for _ in 0..row {
        press(app, KeyCode::Char('j'))?;
    }
    app.refresh()
}

#[test]
fn turning_jira_off_skips_its_milestone_and_unlocks_dev() -> Result<()> {
    let mut fixture = support::Fixture::new(1)?;
    let app = &mut fixture.app;
    assert_eq!(app.detail.milestone, Milestone::Jira);
    press(app, KeyCode::Esc)?;

    open_settings_row(app, 1)?;
    press(app, KeyCode::Enter)?;
    assert!(!app.store.settings()?.jira);
    press(app, KeyCode::Esc)?;

    press(app, KeyCode::Enter)?;
    assert_eq!(app.screen, Screen::Detail);
    assert_eq!(app.detail.milestone, Milestone::Dev);
    assert_eq!(app.actions(), vec![DetailAction::Implement]);
    press(app, KeyCode::Char('k'))?;
    assert_eq!(app.detail.milestone, Milestone::Spec);
    press(app, KeyCode::Char('j'))?;
    assert_eq!(app.detail.milestone, Milestone::Dev);
    press(app, KeyCode::Char('j'))?;
    press(app, KeyCode::Char('j'))?;
    assert_eq!(app.detail.milestone, Milestone::Pr);

    // Turning it back on restores the stage and makes the ticket the next step again.
    press(app, KeyCode::Esc)?;
    open_settings_row(app, 1)?;
    press(app, KeyCode::Enter)?;
    assert!(app.store.settings()?.jira);
    press(app, KeyCode::Esc)?;
    press(app, KeyCode::Enter)?;
    assert_eq!(app.detail.milestone, Milestone::Jira);
    assert_eq!(
        app.actions(),
        [DetailAction::CreateJira, DetailAction::Jira]
    );
    Ok(())
}

#[test]
fn archive_list_restores_with_progress_and_deletes_only_after_confirmation() -> Result<()> {
    let mut fixture = support::Fixture::new(2)?;
    let app = &mut fixture.app;
    let record = app.current().unwrap().clone();
    let archive_selected = |app: &mut App| -> Result<()> {
        press(app, KeyCode::Char('a'))?;
        press(app, KeyCode::Enter)?;
        assert!(app.records.is_empty());
        open_settings_row(app, 2)?;
        assert_eq!(app.archive.records.len(), 1);
        press(app, KeyCode::Enter)?;
        assert_eq!(app.screen, Screen::Archive);
        Ok(())
    };
    press(app, KeyCode::Esc)?;
    archive_selected(app)?;
    press(app, KeyCode::Char('d'))?;
    press(app, KeyCode::Esc)?;
    assert_eq!(app.store.archived()?.len(), 1);
    assert!(record.spec_path.is_file());

    press(app, KeyCode::Char('r'))?;
    assert_eq!(app.notice.text(), "Restored Payment retries");
    assert_eq!(app.records.len(), 1);
    assert_eq!(app.records[0].id, record.id);
    assert_eq!(app.records[0].jira.key.as_deref(), Some("PAY-123"));
    press(app, KeyCode::Esc)?;
    assert_eq!(app.screen, Screen::Settings);
    assert_eq!(app.settings.selected, crate::ui::settings::Row::Archive);
    press(app, KeyCode::Esc)?;
    assert_eq!(app.screen, Screen::List);

    archive_selected(app)?;
    press(app, KeyCode::Char('d'))?;
    press(app, KeyCode::Enter)?;
    assert_eq!(
        app.notice.text(),
        "Deleted Payment retries; file moved to the macOS Trash"
    );
    assert!(!record.spec_path.exists());
    assert!(app.store.path().join("test-macos-trash").is_dir());
    assert!(app.store.archived()?.is_empty());
    assert!(app.store.get(&record.id).is_err());
    assert_eq!(app.store.scan()?.imported, 0);
    Ok(())
}

#[test]
fn a_prompt_is_dropped_only_when_its_own_item_leaves_the_inbox() -> Result<()> {
    let mut fixture = support::Fixture::new(1)?;
    let app = &mut fixture.app;
    press(app, KeyCode::Esc)?;
    let first = app.current().unwrap().id.clone();
    let folder = app.config.sources[0].path.clone();
    fs::write(folder.join("second.md"), "# Second spec\n")?;
    app.store.scan()?;
    app.refresh()?;
    let second = app
        .records
        .iter()
        .find(|record| record.id != first)
        .unwrap()
        .id
        .clone();

    // Typing a new spec's details must survive some other item being archived elsewhere.
    app.begin(Prompt::LaunchWorkspace {
        profile: Profile::Opus,
    });
    app.modal.input = "my-workspace".into();
    app.store.archive(&first)?;
    app.refresh()?;
    assert!(matches!(
        app.modal.prompt,
        Some(Prompt::LaunchWorkspace { .. })
    ));
    assert_eq!(app.modal.input, "my-workspace");

    // A prompt about an item goes away with that item.
    app.begin(Prompt::Jira { id: second.clone() });
    app.modal.input = "PAY-".into();
    app.store.archive(&second)?;
    app.refresh()?;
    assert!(app.modal.prompt.is_none());
    assert!(app.modal.input.is_empty());
    Ok(())
}
