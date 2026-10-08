use super::*;
use crate::store::SpecSource;

struct Fixture {
    root: PathBuf,
    app: App,
}

impl Fixture {
    fn new() -> Result<Self> {
        let root = std::env::temp_dir().join(format!("inbox-chrome-{}", Uuid::new_v4()));
        let source = root.join("context");
        let domain = source.join("missions/zeller");
        fs::create_dir_all(&domain)?;
        fs::write(
            domain.join("payment.md"),
            "# Payment retries\n\nRetry transient failures.\n",
        )?;
        let store = Store::new(root.join("data"));
        let mut settings = store.settings()?;
        settings.sources.push(SpecSource::new(source)?);
        store.save_settings(&settings)?;
        store.scan()?;
        Ok(Self {
            root,
            app: App::new(store)?,
        })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

use super::support::lines as render;

fn assert_header(app: &mut App, suffix: &str, label: &str) -> Result<()> {
    for (width, height) in [(40, 18), (100, 35)] {
        let lines = render(app, width, height)?;
        assert!(
            lines[1].trim_end().ends_with(suffix),
            "{label}: {}",
            lines[1]
        );
    }
    Ok(())
}

#[test]
fn every_view_keeps_the_same_inbox_header_anchor_and_current_crumb() -> Result<()> {
    let mut fixture = Fixture::new()?;
    assert_header(&mut fixture.app, "Inbox", "list")?;
    fixture.app.screen = Screen::Detail;
    assert_header(&mut fixture.app, "Payment retries", "detail")?;
    let lines = render(&mut fixture.app, 100, 35)?;
    assert!(lines[1].contains("missions"));
    assert!(lines[1].contains("zeller"));
    let detail_header = lines[1].clone();
    fixture.app.screen = Screen::Reader;
    assert_header(&mut fixture.app, "FULL SPEC", "reader")?;
    super::super::settings::open(&mut fixture.app)?;
    assert_header(&mut fixture.app, "Settings", "settings")?;
    fixture.app.screen = Screen::ScanResult;
    fixture.app.scan_issues = vec!["Mock unavailable source".into()];
    assert_header(&mut fixture.app, "Scan results", "scan-results")?;
    let lines = render(&mut fixture.app, 100, 35)?;
    assert!(lines[1].contains("Settings"));
    assert!(!detail_header.contains("context"));
    Ok(())
}

#[test]
fn client_choosers_and_new_spec_prompts_keep_the_shared_header() -> Result<()> {
    let mut fixture = Fixture::new()?;
    fixture
        .app
        .choose_client(ChoicePurpose::NewSpec, vec![Profile::Codex]);
    assert_header(&mut fixture.app, "New spec", "new-client")?;
    handle_key(
        &mut fixture.app,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    )?;
    assert!(matches!(
        fixture.app.prompt,
        Some(Prompt::LaunchWorkspace { .. })
    ));
    assert_header(&mut fixture.app, "New spec", "new-workspace")?;
    fixture.app.prompt = None;
    fixture.app.screen = Screen::Detail;
    let id = fixture.app.current().unwrap().id.clone();
    fixture
        .app
        .choose_client(ChoicePurpose::Refine { id }, vec![Profile::Codex]);
    assert_header(&mut fixture.app, "Refine", "refine-client")?;
    Ok(())
}

#[test]
fn long_domain_paths_keep_inbox_and_terminal_crumb_readable() -> Result<()> {
    let mut fixture = Fixture::new()?;
    let domain = fixture
        .root
        .join("context/an unusually long domain/another long subdomain/a third long domain");
    fs::create_dir_all(&domain)?;
    fs::write(domain.join("brief.md"), "# Brief\n\nMock spec.\n")?;
    fixture.app.store.scan()?;
    fixture.app.refresh()?;
    fixture.app.selected = fixture
        .app
        .records
        .iter()
        .position(|record| record.title == "Brief")
        .unwrap();
    fixture.app.tree.focus_record(fixture.app.selected);
    fixture.app.screen = Screen::Detail;
    assert_header(&mut fixture.app, "Brief", "long-detail")?;
    fixture.app.screen = Screen::Reader;
    assert_header(&mut fixture.app, "FULL SPEC", "long-reader")?;
    let lines = render(&mut fixture.app, 40, 18)?;
    assert!(lines[1].contains('…'));
    Ok(())
}

#[test]
fn list_inbox_is_accented_but_ancestor_inbox_is_muted_in_other_views() -> Result<()> {
    let mut fixture = Fixture::new()?;
    assert_header(&mut fixture.app, "Inbox", "accent-list")?;
    fixture.app.screen = Screen::Detail;
    assert_header(&mut fixture.app, "Payment retries", "accent-detail")?;
    fixture.app.screen = Screen::Reader;
    assert_header(&mut fixture.app, "FULL SPEC", "accent-reader")?;
    super::super::settings::open(&mut fixture.app)?;
    assert_header(&mut fixture.app, "Settings", "accent-settings")?;
    Ok(())
}
