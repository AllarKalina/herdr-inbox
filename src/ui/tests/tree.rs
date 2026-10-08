use super::*;
use crate::store::SpecSource;

struct Fixture {
    root: PathBuf,
    app: App,
}

impl Fixture {
    fn new() -> Result<Self> {
        let root = std::env::temp_dir().join(format!("inbox-tree-{}", Uuid::new_v4()));
        let context = root.join("context");
        for (domain, prefix) in [("designs", "design"), ("missions/zeller", "mission")] {
            let directory = context.join(domain);
            fs::create_dir_all(&directory)?;
            for number in 1..=5 {
                fs::write(
                    directory.join(format!("{prefix}-{number}.md")),
                    format!("# {prefix}-{number}\n\nMock local spec.\n"),
                )?;
            }
        }
        fs::write(context.join("designs/preview.html"), "<h1>preview</h1>")?;
        let store = Store::new(root.join("data"));
        let mut settings = store.settings()?;
        let mut source = SpecSource::new(context)?;
        source.include.push("**/*.html".into());
        settings.sources.push(source);
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

fn press(app: &mut App, code: KeyCode) -> Result<()> {
    handle_key(app, KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn focus(app: &mut App, label: &str) -> Result<()> {
    let index = app
        .tree
        .rows
        .iter()
        .position(|row| row.label == label)
        .unwrap();
    for _ in 0..app.tree.rows.len() {
        press(app, KeyCode::Up)?;
    }
    for _ in 0..index {
        press(app, KeyCode::Down)?;
    }
    assert_eq!(app.tree.rows[app.tree.focused].label, label);
    Ok(())
}

type Rendered = (Vec<String>, Vec<Vec<ratatui::buffer::Cell>>);

fn render(app: &mut App, width: u16, height: u16) -> Result<Rendered> {
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
        let name = format!("tree-{width}x{height}");
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
    Ok((lines, cells))
}

#[test]
fn tree_groups_domains_and_distinguishes_folder_markdown_and_html_icons() -> Result<()> {
    let mut fixture = Fixture::new()?;
    assert_eq!(fixture.app.records.len(), 11);
    let rows = &fixture.app.tree.rows;
    let row = |label: &str| rows.iter().find(|row| row.label == label).unwrap();
    assert!(
        !rows
            .iter()
            .any(|row| row.label == "context" || row.label == "Inbox specs")
    );
    assert_eq!(row("designs").depth, 0);
    assert_eq!(row("missions").depth, row("designs").depth);
    assert_eq!(row("zeller").depth, row("missions").depth + 1);
    assert_eq!(row("mission-1").depth, row("zeller").depth + 1);
    assert!(row("mission-1").prefix.contains('│') || row("mission-1").prefix.contains('├'));
    assert!(row("designs").record_index.is_none());
    assert!(row("design-1").record_index.is_some());
    assert!(
        rows.iter()
            .all(|row| row.prefix.chars().count() == row.depth * 3)
    );
    let (lines, cells) = render(&mut fixture.app, 110, 30)?;
    let find = |text: &str| lines.iter().position(|line| line.contains(text)).unwrap();
    let folder = find("designs");
    let markdown = find("design-1");
    let html = find("preview");
    let private_icons = |line: &str| {
        line.chars()
            .filter(|ch| {
                ('\u{e000}'..='\u{f8ff}').contains(ch) || ('\u{f0000}'..='\u{ffffd}').contains(ch)
            })
            .collect::<Vec<_>>()
    };
    let folder_icons = private_icons(&lines[folder]);
    let markdown_icons = private_icons(&lines[markdown]);
    let html_icons = private_icons(&lines[html]);
    assert!(!folder_icons.is_empty());
    assert!(!markdown_icons.is_empty());
    assert!(!html_icons.is_empty());
    assert_ne!(folder_icons, markdown_icons);
    assert_ne!(markdown_icons, html_icons);
    let header_row = lines
        .iter()
        .position(|line| line.contains("Spec") && line.contains("Jira"))
        .unwrap();
    let status_column = cells[header_row]
        .iter()
        .position(|cell| cell.symbol() == "S")
        .unwrap();
    assert!(
        cells[folder][status_column..]
            .iter()
            .all(|cell| cell.symbol().trim().is_empty())
    );
    let done_column = |row: usize| {
        cells[row]
            .iter()
            .position(|cell| cell.symbol() == "✓")
            .unwrap()
    };
    assert_eq!(done_column(markdown), done_column(html));
    assert!(
        cells
            .iter()
            .flatten()
            .all(|cell| cell.bg == Color::Reset || cell.bg == Color::Cyan)
    );
    Ok(())
}

#[test]
fn tree_keyboard_collapses_domains_and_opens_the_correct_file_without_archiving_folders()
-> Result<()> {
    let mut fixture = Fixture::new()?;
    focus(&mut fixture.app, "missions")?;
    assert!(fixture.app.current().is_none());
    press(&mut fixture.app, KeyCode::Char('a'))?;
    assert!(fixture.app.prompt.is_none());
    let count = fixture.app.tree.rows.len();
    press(&mut fixture.app, KeyCode::Enter)?;
    assert_eq!(fixture.app.screen, Screen::List);
    assert!(fixture.app.tree.rows.len() < count);
    assert!(
        !fixture
            .app
            .tree
            .rows
            .iter()
            .any(|row| row.label == "mission-1")
    );
    press(&mut fixture.app, KeyCode::Right)?;
    assert_eq!(fixture.app.tree.rows.len(), count);
    press(&mut fixture.app, KeyCode::Right)?;
    assert_eq!(
        fixture.app.tree.rows[fixture.app.tree.focused].label,
        "zeller"
    );
    press(&mut fixture.app, KeyCode::Left)?;
    assert!(
        !fixture
            .app
            .tree
            .rows
            .iter()
            .any(|row| row.label == "mission-1")
    );
    press(&mut fixture.app, KeyCode::Left)?;
    assert_eq!(
        fixture.app.tree.rows[fixture.app.tree.focused].label,
        "missions"
    );
    focus(&mut fixture.app, "zeller")?;
    press(&mut fixture.app, KeyCode::Enter)?;
    focus(&mut fixture.app, "mission-3")?;
    let id = fixture.app.current().unwrap().id.clone();
    press(&mut fixture.app, KeyCode::Enter)?;
    assert_eq!(fixture.app.screen, Screen::Detail);
    assert_eq!(fixture.app.current().unwrap().id, id);
    press(&mut fixture.app, KeyCode::Esc)?;
    assert_eq!(
        fixture.app.tree.rows[fixture.app.tree.focused].label,
        "mission-3"
    );
    assert_eq!(fixture.app.store.list()?.len(), 11);
    Ok(())
}

#[test]
fn tree_mouse_uses_visible_folder_rows_and_keeps_file_uuid_after_refresh() -> Result<()> {
    let mut fixture = Fixture::new()?;
    let (lines, _) = render(&mut fixture.app, 110, 30)?;
    let design_row = lines
        .iter()
        .position(|line| line.contains("designs"))
        .unwrap() as u16;
    let mouse = |kind, row| MouseEvent {
        kind,
        column: 8,
        row,
        modifiers: KeyModifiers::NONE,
    };
    handle_mouse(
        &mut fixture.app,
        mouse(MouseEventKind::Moved, design_row),
        30,
    )?;
    assert!(fixture.app.current().is_none());
    assert_eq!(
        fixture.app.tree.rows[fixture.app.tree.focused].label,
        "designs"
    );
    handle_mouse(
        &mut fixture.app,
        mouse(MouseEventKind::Down(MouseButton::Left), design_row),
        30,
    )?;
    let folder_key = fixture.app.tree.rows[fixture.app.tree.focused].key.clone();
    fixture.app.refresh()?;
    assert_eq!(
        fixture.app.tree.rows[fixture.app.tree.focused].key,
        folder_key
    );
    assert!(fixture.app.current().is_none());
    assert!(
        !fixture
            .app
            .tree
            .rows
            .iter()
            .any(|row| row.label == "design-1")
    );
    let (lines, _) = render(&mut fixture.app, 110, 30)?;
    let file_row = lines
        .iter()
        .position(|line| line.contains("mission-3"))
        .unwrap() as u16;
    handle_mouse(&mut fixture.app, mouse(MouseEventKind::Moved, file_row), 30)?;
    let id = fixture.app.current().unwrap().id.clone();
    fixture.app.store.update(
        &id,
        Change::Jira {
            key: "TREE-3".into(),
            url: None,
        },
    )?;
    fixture.app.refresh()?;
    assert_eq!(fixture.app.current().unwrap().id, id);
    assert_eq!(
        fixture.app.current().unwrap().jira.key.as_deref(),
        Some("TREE-3")
    );
    assert!(
        !fixture
            .app
            .tree
            .rows
            .iter()
            .any(|row| row.label == "design-1")
    );
    press(&mut fixture.app, KeyCode::Enter)?;
    assert_eq!(fixture.app.screen, Screen::Detail);
    assert_eq!(fixture.app.current().unwrap().id, id);
    Ok(())
}

#[test]
fn tree_deep_long_paths_keep_compact_statuses_and_footer_visible() -> Result<()> {
    let mut fixture = Fixture::new()?;
    let directory = fixture
        .root
        .join("context/missions/zeller/a very long domain/b very long domain/c very long domain");
    fs::create_dir_all(&directory)?;
    fs::write(
        directory.join("A long nested spec title that exceeds its available column.md"),
        "# A long nested spec title that exceeds its available column\n",
    )?;
    fixture.app.store.scan()?;
    fixture.app.refresh()?;
    focus(
        &mut fixture.app,
        "A long nested spec title that exceeds its available column",
    )?;
    let id = fixture.app.current().unwrap().id.clone();
    for (width, height) in [(40, 18), (60, 24), (110, 30)] {
        let (lines, cells) = render(&mut fixture.app, width, height)?;
        let row = cells
            .iter()
            .position(|row| row.iter().any(|cell| cell.bg == Color::Cyan))
            .unwrap();
        assert!(cells[row].iter().any(|cell| cell.symbol() == "✓"));
        assert!(cells[row].iter().any(|cell| cell.symbol() == "→"));
        let header_row = lines
            .iter()
            .position(|line| {
                line.contains("S J D P") || (line.contains("Spec") && line.contains("Jira"))
            })
            .unwrap();
        let status_column = cells[header_row]
            .iter()
            .position(|cell| cell.symbol() == "S")
            .unwrap();
        assert_eq!(cells[row][status_column].symbol(), "✓");
        let footer = lines
            .iter()
            .rev()
            .find(|line| line.contains("s settings"))
            .unwrap();
        assert!(footer.contains("Enter") || footer.contains('↵'));
        assert!(footer.contains("n new"));
        assert!(!lines.join("\n").contains("S scan"));
        assert_eq!(fixture.app.current().unwrap().id, id);
    }
    press(&mut fixture.app, KeyCode::Enter)?;
    assert_eq!(fixture.app.screen, Screen::Detail);
    assert_eq!(fixture.app.current().unwrap().id, id);
    Ok(())
}

#[test]
fn unselected_records_do_not_form_loose_rows() -> Result<()> {
    let root = std::env::temp_dir().join(format!("inbox-tree-unselected-{}", Uuid::new_v4()));
    let store = configured_store(root.join("data"))?;
    store.start("Old first", None, None)?;
    store.start("Old second", None, None)?;
    let mut settings = store.settings()?;
    settings.sources.clear();
    store.save_settings(&settings)?;
    let mut fixture = Fixture {
        root,
        app: App::new(store)?,
    };
    assert!(fixture.app.records.is_empty());
    assert!(fixture.app.tree.rows.is_empty());
    assert_eq!(fixture.app.screen, Screen::Settings);
    let (lines, _) = render(&mut fixture.app, 100, 24)?;
    assert!(!lines.iter().any(|line| line.contains("Inbox specs")));
    assert!(!lines.iter().any(|line| line.contains("Old first")));
    assert!(!lines.iter().any(|line| line.contains("Old second")));
    Ok(())
}
