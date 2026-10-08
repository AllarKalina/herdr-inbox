use super::*;
use crate::store::SpecSource;

struct Fixture {
    root: PathBuf,
    source: PathBuf,
    outside: Vec<Record>,
    app: App,
}

impl Fixture {
    fn new() -> Result<Self> {
        let root = std::env::temp_dir().join(format!("inbox-scope-{}", Uuid::new_v4()));
        let source = root.join("chosen/domain");
        fs::create_dir_all(&source)?;
        fs::write(
            source.join("selected.md"),
            "# Selected folder spec\nOnly selected content.\n",
        )?;
        let source = source.parent().unwrap().canonicalize()?;
        let store = Store::new(root.join("data"));
        let mut settings = store.settings()?;
        settings.sources.push(SpecSource::new(source.clone())?);
        store.save_settings(&settings)?;
        store.scan()?;
        let template = store.list()?.pop().unwrap();
        let mut outside = Vec::new();
        for (title, directory) in [
            ("Historical managed spec", root.join("data/specs")),
            ("Unselected user spec", root.join("elsewhere")),
        ] {
            fs::create_dir_all(&directory)?;
            let mut record = template.clone();
            record.id = Uuid::new_v4().to_string();
            record.title = title.into();
            record.spec_path = directory.join(format!("{}.md", record.id));
            // Cached source metadata must never authorize a file outside the selected root.
            record.source_id = Some(settings.sources[0].id.clone());
            record.source_relative_path = Some(PathBuf::from("domain/selected.md"));
            fs::write(&record.spec_path, format!("# {title}\nOutside content.\n"))?;
            fs::write(
                store
                    .path()
                    .join("items")
                    .join(format!("{}.json", record.id)),
                serde_json::to_vec_pretty(&record)?,
            )?;
            outside.push(record);
        }
        Ok(Self {
            root,
            source,
            outside,
            app: App::new(store)?,
        })
    }

    fn focus_spec(&mut self) {
        self.app.tree.focus_record(0);
        self.app.sync_tree_selection();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn press(app: &mut App, code: KeyCode) -> Result<()> {
    handle_key(app, KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn render(app: &mut App) -> Result<String> {
    let mut terminal = Terminal::new(TestBackend::new(100, 28))?;
    terminal.draw(|frame| draw::draw(frame, app))?;
    Ok(terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect())
}

#[test]
fn selected_folder_is_the_only_authority_for_list_open_and_archive() -> Result<()> {
    let mut fixture = Fixture::new()?;
    assert_eq!(fixture.app.store.list()?.len(), 3);
    assert_eq!(fixture.app.records.len(), 1);
    assert_eq!(fixture.app.records[0].title, "Selected folder spec");
    assert!(
        fixture.app.records[0]
            .spec_path
            .starts_with(&fixture.source)
    );
    assert_eq!(
        fixture
            .app
            .tree
            .rows
            .iter()
            .filter(|row| row.record_index.is_some())
            .count(),
        1
    );
    let text = render(&mut fixture.app)?;
    assert!(text.contains("Selected folder spec"));
    for outside in &fixture.outside {
        assert!(!text.contains(&outside.title));
    }
    fixture.focus_spec();
    press(&mut fixture.app, KeyCode::Enter)?;
    assert_eq!(fixture.app.screen, Screen::Detail);
    assert_eq!(fixture.app.current().unwrap().title, "Selected folder spec");
    press(&mut fixture.app, KeyCode::Char('r'))?;
    assert_eq!(fixture.app.screen, Screen::Reader);
    assert!(render(&mut fixture.app)?.contains("Only selected content."));
    press(&mut fixture.app, KeyCode::Esc)?;
    press(&mut fixture.app, KeyCode::Esc)?;
    assert_eq!(fixture.app.screen, Screen::List);
    press(&mut fixture.app, KeyCode::Char('a'))?;
    press(&mut fixture.app, KeyCode::Enter)?;
    assert!(fixture.app.records.is_empty());
    for outside in &fixture.outside {
        assert!(outside.spec_path.is_file());
        assert!(fixture.app.store.get(&outside.id).is_ok());
        assert!(
            !fixture
                .app
                .store
                .path()
                .join("trash/items")
                .join(format!("{}.json", outside.id))
                .exists()
        );
    }
    Ok(())
}

#[test]
fn removing_sources_closes_stale_detail_and_requires_folder_selection() -> Result<()> {
    let mut fixture = Fixture::new()?;
    fixture.focus_spec();
    press(&mut fixture.app, KeyCode::Enter)?;
    fixture.app.begin(Prompt::Jira {
        id: fixture.app.current().unwrap().id.clone(),
    });
    let mut settings = fixture.app.store.settings()?;
    settings.sources.clear();
    fixture.app.store.save_settings(&settings)?;
    fixture.app.refresh()?;
    assert!(fixture.app.records.is_empty());
    assert!(fixture.app.tree.rows.is_empty());
    assert!(fixture.app.current().is_none());
    assert!(fixture.app.prompt.is_none());
    assert_eq!(fixture.app.screen, Screen::Settings);
    let restarted = App::new(Store::new(fixture.app.store.path().to_path_buf()))?;
    assert!(restarted.records.is_empty());
    assert_eq!(restarted.screen, Screen::Settings);
    assert_eq!(restarted.store.list()?.len(), 3);
    Ok(())
}

#[test]
fn switching_source_dismisses_reader_and_uses_only_the_new_folder() -> Result<()> {
    let mut fixture = Fixture::new()?;
    fixture.focus_spec();
    press(&mut fixture.app, KeyCode::Enter)?;
    press(&mut fixture.app, KeyCode::Char('r'))?;
    let replacement = fixture.root.join("replacement");
    fs::create_dir_all(&replacement)?;
    fs::write(replacement.join("new.md"), "# Replacement folder spec\n")?;
    let mut settings = fixture.app.store.settings()?;
    settings.sources = vec![SpecSource::new(replacement.clone())?];
    fixture.app.store.save_settings(&settings)?;
    fixture.app.store.scan()?;
    fixture.app.refresh()?;
    assert_eq!(fixture.app.screen, Screen::List);
    assert_eq!(fixture.app.records.len(), 1);
    assert_eq!(fixture.app.records[0].title, "Replacement folder spec");
    assert!(
        fixture.app.records[0]
            .spec_path
            .starts_with(replacement.canonicalize()?)
    );
    let text = render(&mut fixture.app)?;
    assert!(!text.contains("Selected folder spec"));
    assert!(!text.contains("Outside content"));
    Ok(())
}

#[test]
fn a_deleted_spec_is_not_retained_as_a_visible_metadata_row() -> Result<()> {
    let mut fixture = Fixture::new()?;
    fixture.focus_spec();
    press(&mut fixture.app, KeyCode::Enter)?;
    let record = fixture.app.current().unwrap().clone();
    fs::remove_file(&record.spec_path)?;
    fixture.app.refresh()?;
    assert!(fixture.app.records.is_empty());
    assert!(fixture.app.tree.rows.is_empty());
    assert!(fixture.app.current().is_none());
    assert_ne!(fixture.app.screen, Screen::Detail);
    assert!(fixture.app.store.get(&record.id).is_ok());
    Ok(())
}
