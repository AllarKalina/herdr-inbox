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
        let name = format!("chrome-{label}-{width}x{height}");
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
    assert!(
        lines[0].trim().is_empty(),
        "{label}: header moved to outer edge"
    );
    assert!(lines[1].starts_with("  Inbox"), "{label}: {}", lines[1]);
    assert!(
        lines[2].trim().is_empty(),
        "{label}: header spacing changed"
    );
    assert!(
        !lines.iter().any(|line| line.contains("← Inbox")),
        "{label}: duplicate back-arrow header"
    );
    let header = &cells[1];
    let final_start = header
        .iter()
        .rposition(|cell| cell.symbol() == "/")
        .map_or(2, |separator| separator + 2);
    let final_end = header
        .iter()
        .rposition(|cell| !cell.symbol().trim().is_empty())
        .unwrap();
    for (column, cell) in header.iter().enumerate().take(final_end + 1).skip(2) {
        if cell.symbol().trim().is_empty() {
            continue;
        }
        if column < final_start {
            assert_eq!(
                cell.fg,
                Color::Gray,
                "{label}: ancestor at {column} must be muted"
            );
            assert!(
                !cell.modifier.contains(ratatui::style::Modifier::BOLD),
                "{label}: ancestor at {column} must not be bold"
            );
        } else {
            assert_eq!(
                cell.fg,
                Color::LightCyan,
                "{label}: current crumb at {column} must be teal"
            );
            assert!(
                cell.modifier.contains(ratatui::style::Modifier::BOLD),
                "{label}: current crumb at {column} must be bold"
            );
        }
    }
    Ok(lines)
}

fn assert_header(app: &mut App, suffix: &str, label: &str) -> Result<()> {
    for (width, height) in [(40, 18), (100, 35)] {
        let lines = render(app, width, height, label)?;
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
    let lines = render(&mut fixture.app, 100, 35, "detail-domains")?;
    assert!(lines[1].contains("missions"));
    assert!(lines[1].contains("zeller"));
    let detail_header = lines[1].clone();
    fixture.app.screen = Screen::Reader;
    assert_header(&mut fixture.app, "FULL SPEC", "reader")?;
    super::super::settings::open(&mut fixture.app)?;
    assert_header(&mut fixture.app, "Settings", "settings")?;
    fixture.app.settings.first_use = true;
    assert_header(&mut fixture.app, "Connect your specs", "first-use")?;
    fixture.app.settings.first_use = false;
    fixture.app.screen = Screen::ScanResult;
    fixture.app.scan_issues = vec!["Mock unavailable source".into()];
    assert_header(&mut fixture.app, "Scan results", "scan-results")?;
    let lines = render(&mut fixture.app, 100, 35, "scan-results-domains")?;
    assert!(lines[1].contains("Settings"));
    fixture.app.screen = Screen::Trash;
    fixture.app.trash = fixture.app.records.clone();
    assert_header(&mut fixture.app, "Archived items", "archived-items")?;
    let lines = render(&mut fixture.app, 100, 35, "archive-domains")?;
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
    let lines = render(&mut fixture.app, 40, 18, "long-reader-final")?;
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
