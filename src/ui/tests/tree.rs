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
        .list
        .tree
        .rows
        .iter()
        .position(|row| row.label == label)
        .unwrap();
    for _ in 0..app.list.tree.rows.len() {
        press(app, KeyCode::Up)?;
    }
    for _ in 0..index {
        press(app, KeyCode::Down)?;
    }
    assert_eq!(app.list.tree.rows[app.list.tree.focused].label, label);
    Ok(())
}

type Rendered = (Vec<String>, Vec<Vec<ratatui::buffer::Cell>>);

fn render(app: &mut App, width: u16, height: u16) -> Result<Rendered> {
    let cells = super::support::cells(app, width, height)?;
    Ok((super::support::text(&cells), cells))
}

#[test]
fn tree_keyboard_collapses_domains_and_opens_the_correct_file_without_archiving_folders()
-> Result<()> {
    let mut fixture = Fixture::new()?;
    focus(&mut fixture.app, "missions")?;
    assert!(fixture.app.current().is_none());
    press(&mut fixture.app, KeyCode::Char('a'))?;
    assert!(fixture.app.modal.prompt.is_none());
    let count = fixture.app.list.tree.rows.len();
    press(&mut fixture.app, KeyCode::Enter)?;
    assert_eq!(fixture.app.screen, Screen::List);
    assert!(fixture.app.list.tree.rows.len() < count);
    assert!(
        !fixture
            .app
            .list
            .tree
            .rows
            .iter()
            .any(|row| row.label == "mission-1")
    );
    press(&mut fixture.app, KeyCode::Right)?;
    assert_eq!(fixture.app.list.tree.rows.len(), count);
    press(&mut fixture.app, KeyCode::Right)?;
    assert_eq!(
        fixture.app.list.tree.rows[fixture.app.list.tree.focused].label,
        "zeller"
    );
    press(&mut fixture.app, KeyCode::Left)?;
    assert!(
        !fixture
            .app
            .list
            .tree
            .rows
            .iter()
            .any(|row| row.label == "mission-1")
    );
    press(&mut fixture.app, KeyCode::Left)?;
    assert_eq!(
        fixture.app.list.tree.rows[fixture.app.list.tree.focused].label,
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
        fixture.app.list.tree.rows[fixture.app.list.tree.focused].label,
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
    handle_mouse(&mut fixture.app, mouse(MouseEventKind::Moved, design_row))?;
    assert!(fixture.app.current().is_none());
    assert_eq!(
        fixture.app.list.tree.rows[fixture.app.list.tree.focused].label,
        "designs"
    );
    handle_mouse(
        &mut fixture.app,
        mouse(MouseEventKind::Down(MouseButton::Left), design_row),
    )?;
    let folder_key = fixture.app.list.tree.rows[fixture.app.list.tree.focused]
        .key
        .clone();
    fixture.app.refresh()?;
    assert_eq!(
        fixture.app.list.tree.rows[fixture.app.list.tree.focused].key,
        folder_key
    );
    assert!(fixture.app.current().is_none());
    assert!(
        !fixture
            .app
            .list
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
    handle_mouse(&mut fixture.app, mouse(MouseEventKind::Moved, file_row))?;
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
            .list
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
