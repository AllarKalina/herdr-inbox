use super::*;
use crate::store::Change;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

mod chrome;
mod delight;
mod feedback;
mod footer;
mod golden;
mod list;
mod proximity;
mod quest;
mod refinement;
mod resize;
mod scope;
mod settings_page;
mod sources;
mod support;
mod timeline;
mod tree;

fn configured_store(root: PathBuf) -> Result<Store> {
    let store = Store::new(root.clone());
    let specs = root.join("specs");
    fs::create_dir_all(&specs)?;
    let mut settings = store.settings()?;
    settings.sources.push(crate::store::SpecSource::new(specs)?);
    store.save_settings(&settings)?;
    Ok(store)
}

#[test]
fn delete_requires_second_enter_and_esc_cancels() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-ui-{}", Uuid::new_v4()));
    let store = configured_store(root.clone())?;
    let record = store.start("Keep until confirmed", None, None)?;
    let mut app = App::new(store)?;
    let press = |app: &mut App, code| handle_key(app, KeyEvent::new(code, KeyModifiers::NONE));

    press(&mut app, KeyCode::Char('a'))?;
    assert!(matches!(app.prompt, Some(Prompt::Archive { .. })));
    press(&mut app, KeyCode::Esc)?;
    assert!(app.store.get(&record.id).is_ok());

    press(&mut app, KeyCode::Char('a'))?;
    press(&mut app, KeyCode::Char('x'))?;
    assert!(app.store.get(&record.id).is_ok());
    assert!(app.input.is_empty());
    press(&mut app, KeyCode::Enter)?;
    assert!(app.store.list()?.is_empty());
    let footer = draw::footer_text(&app, 100);
    assert!(footer.starts_with("Item archived\n"));
    assert!(footer.contains("Enter open · n new · a archive · s settings"));
    assert!(
        root.join("trash/items")
            .join(format!("{}.json", record.id))
            .is_file()
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn list_stage_shortcuts_no_longer_start_actions() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-list-keys-{}", Uuid::new_v4()));
    let store = configured_store(root.clone())?;
    let record = store.start("Keep in progress", None, None)?;
    let mut app = App::new(store)?;

    for key in ['d', 'S', 'u', 'f', 't', 'J', 'i', 'p', 'q'] {
        assert!(!handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE)
        )?);
        assert!(app.prompt.is_none());
    }
    assert_eq!(app.store.get(&record.id)?.spec, "in_progress");
    assert!(draw::footer_text(&app, 100).contains("Enter open · n new · a archive · s settings"));
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn selected_spec_opens_detail_and_full_reader_then_returns() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-detail-{}", Uuid::new_v4()));
    let store = configured_store(root.clone())?;
    let record = store.start("Payment retries", None, None)?;
    fs::write(
        &record.spec_path,
        "# Payment retries\n\nRetry only transient failures.\n",
    )?;
    let mut app = App::new(store)?;
    let press = |app: &mut App, code| handle_key(app, KeyEvent::new(code, KeyModifiers::NONE));

    press(&mut app, KeyCode::Enter)?;
    assert_eq!(app.screen, Screen::Detail);
    let mut terminal = Terminal::new(TestBackend::new(100, 35))?;
    terminal.draw(|frame| draw::draw(frame, &mut app))?;
    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(rendered.contains("SPEC"));
    assert!(!rendered.contains("SPEC BRIEF"));
    assert!(!rendered.contains("v full spec"));
    assert!(rendered.contains("Retry only transient failures"));
    assert!(rendered.contains("PROGRESS"));
    assert!(rendered.contains("Seal the spec"));
    assert!(!rendered.contains("STAGE & LINKS"));
    assert!(!rendered.contains("Local spec"));
    let lines: Vec<String> = terminal
        .backend()
        .buffer()
        .content()
        .chunks(100)
        .map(|cells| cells.iter().map(|cell| cell.symbol()).collect())
        .collect();
    let heading_column = lines.iter().find_map(|line| line.find("SPEC")).unwrap();
    let content_column = lines
        .iter()
        .find_map(|line| line.find("# Payment retries"))
        .unwrap();
    let progress_column = lines.iter().find_map(|line| line.find("PROGRESS")).unwrap();
    assert_eq!(heading_column, content_column);
    assert!(progress_column > content_column + 30);
    let milestone_rows: Vec<usize> = [
        ("SPEC", "active"),
        ("JIRA", "wait"),
        ("DEV", "locked"),
        ("PR", "locked"),
    ]
    .iter()
    .map(|(label, status)| {
        lines
            .iter()
            .position(|line| line.contains(label) && line.contains(status))
            .unwrap()
    })
    .collect();
    assert!(milestone_rows.windows(2).all(|pair| pair[0] < pair[1]));
    let mut narrow = Terminal::new(TestBackend::new(60, 24))?;
    narrow.draw(|frame| draw::draw(frame, &mut app))?;
    let narrow_lines: Vec<String> = narrow
        .backend()
        .buffer()
        .content()
        .chunks(60)
        .map(|cells| cells.iter().map(|cell| cell.symbol()).collect())
        .collect();
    let spec_row = narrow_lines
        .iter()
        .position(|line| line.contains("SPEC"))
        .unwrap();
    let progress_row = narrow_lines
        .iter()
        .position(|line| line.contains("PROGRESS"))
        .unwrap();
    assert!(progress_row > spec_row);

    press(&mut app, KeyCode::Char('v'))?;
    assert_eq!(app.screen, Screen::Detail);
    press(&mut app, KeyCode::Char('r'))?;
    assert_eq!(app.screen, Screen::Reader);
    terminal.draw(|frame| draw::draw(frame, &mut app))?;
    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(rendered.contains("FULL SPEC"));
    assert!(rendered.contains("Inbox / Payment retries / FULL SPEC"));
    assert!(rendered.contains("Retry only transient failures"));
    press(&mut app, KeyCode::Char('r'))?;
    assert_eq!(app.screen, Screen::Detail);
    press(&mut app, KeyCode::Esc)?;
    assert_eq!(app.screen, Screen::List);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn reader_scroll_stops_at_last_wrapped_line_with_two_rows_of_padding() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-scroll-{}", Uuid::new_v4()));
    let store = configured_store(root.clone())?;
    let record = store.start("Long spec", None, None)?;
    fs::write(
        &record.spec_path,
        format!("{}\nLAST LINE", "word ".repeat(300)),
    )?;
    let mut app = App::new(store)?;
    let press = |app: &mut App, code| handle_key(app, KeyEvent::new(code, KeyModifiers::NONE));
    press(&mut app, KeyCode::Enter)?;
    press(&mut app, KeyCode::Char('r'))?;
    let mut terminal = Terminal::new(TestBackend::new(40, 16))?;
    terminal.draw(|frame| draw::draw(frame, &mut app))?;
    assert!(app.reader_max_scroll > 11);
    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(rendered.contains("j/k scroll · Shift+J/K 10 lines"));
    assert!(!rendered.contains("PgUp/PgDn page"));
    press(&mut app, KeyCode::Char('j'))?;
    assert_eq!(app.reader_scroll, 1);
    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT),
    )?;
    assert_eq!(app.reader_scroll, 11);
    press(&mut app, KeyCode::Char('k'))?;
    assert_eq!(app.reader_scroll, 10);
    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT),
    )?;
    assert_eq!(app.reader_scroll, 0);
    press(&mut app, KeyCode::Char('K'))?;
    assert_eq!(app.reader_scroll, 0);

    for _ in 0..50 {
        press(&mut app, KeyCode::PageDown)?;
    }
    assert_eq!(app.reader_scroll, app.reader_max_scroll);
    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT),
    )?;
    assert_eq!(app.reader_scroll, app.reader_max_scroll);
    terminal.draw(|frame| draw::draw(frame, &mut app))?;
    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(rendered.contains("LAST LINE"));
    let rows: Vec<String> = terminal
        .backend()
        .buffer()
        .content()
        .chunks(40)
        .map(|cells| cells.iter().map(|cell| cell.symbol()).collect())
        .collect();
    let last_line = rows
        .iter()
        .position(|row| row.contains("LAST LINE"))
        .unwrap();
    assert!(rows[last_line + 1].trim().is_empty());
    assert!(rows[last_line + 2].trim().is_empty());
    press(&mut app, KeyCode::Down)?;
    assert_eq!(app.reader_scroll, app.reader_max_scroll);
    handle_mouse(
        &mut app,
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 4,
            row: 6,
            modifiers: KeyModifiers::NONE,
        },
    )?;
    assert_eq!(app.reader_scroll, app.reader_max_scroll);

    let mut larger_terminal = Terminal::new(TestBackend::new(100, 30))?;
    larger_terminal.draw(|frame| draw::draw(frame, &mut app))?;
    assert_eq!(app.reader_scroll, 0);
    assert_eq!(app.reader_max_scroll, 0);

    fs::write(&record.spec_path, "Shortened spec")?;
    terminal.draw(|frame| draw::draw(frame, &mut app))?;
    assert_eq!(app.reader_scroll, 0);
    assert_eq!(app.reader_max_scroll, 0);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn progress_actions_require_jira_before_implementation() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-quest-{}", Uuid::new_v4()));
    let store = configured_store(root.clone())?;
    store.start("Payment retries", None, None)?;
    let mut app = App::new(store)?;
    let press = |app: &mut App, code| handle_key(app, KeyEvent::new(code, KeyModifiers::NONE));

    press(&mut app, KeyCode::Enter)?;
    assert_eq!(
        app.actions(),
        vec![
            DetailAction::Finish,
            DetailAction::ReadSpec,
            DetailAction::RefineSpec
        ]
    );
    press(&mut app, KeyCode::Enter)?;
    assert_eq!(app.current().unwrap().spec, "done");
    assert_eq!(app.actions(), vec![DetailAction::Jira]);
    let mut terminal = Terminal::new(TestBackend::new(100, 35))?;
    terminal.draw(|frame| draw::draw(frame, &mut app))?;
    let action = app.action_hitboxes[0];
    handle_mouse(
        &mut app,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: action.x + 1,
            row: action.y,
            modifiers: KeyModifiers::NONE,
        },
    )?;
    assert!(matches!(app.prompt, Some(Prompt::Jira { .. })));
    for ch in "ABC-123".chars() {
        press(&mut app, KeyCode::Char(ch))?;
    }
    press(&mut app, KeyCode::Enter)?;
    assert!(matches!(app.prompt, Some(Prompt::JiraUrl { .. })));
    for ch in "https://jira.example/ABC-123".chars() {
        press(&mut app, KeyCode::Char(ch))?;
    }
    press(&mut app, KeyCode::Enter)?;
    assert_eq!(app.current().unwrap().jira.key.as_deref(), Some("ABC-123"));
    assert_eq!(
        app.current().unwrap().jira.url.as_deref(),
        Some("https://jira.example/ABC-123")
    );
    assert_eq!(app.actions(), vec![DetailAction::Implement]);
    press(&mut app, KeyCode::Enter)?;
    assert!(matches!(app.prompt, Some(Prompt::Agent { .. })));
    press(&mut app, KeyCode::Enter)?;
    assert!(matches!(app.prompt, Some(Prompt::Branch { .. })));
    press(&mut app, KeyCode::Enter)?;
    assert_eq!(app.current().unwrap().implementation.status, "in_progress");
    assert_eq!(app.actions(), vec![DetailAction::Pr]);
    let id = app.current().unwrap().id.clone();
    app.store.update(
        &id,
        Change::Pr {
            url: "https://github.example/pr/1".into(),
        },
    )?;
    app.refresh()?;
    assert_eq!(
        app.actions(),
        vec![DetailAction::ReviewPr, DetailAction::UpdatePr]
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn mouse_hover_selects_a_list_item_before_enter() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-mouse-{}", Uuid::new_v4()));
    let store = configured_store(root.clone())?;
    store.start("First", None, None)?;
    store.start("Second", None, None)?;
    let mut app = App::new(store)?;
    let second_id = app.records[1].id.clone();
    let mut terminal = Terminal::new(TestBackend::new(100, 30))?;
    terminal.draw(|frame| draw::draw(frame, &mut app))?;
    let tree_index = app
        .tree
        .rows
        .iter()
        .position(|row| row.record_index == Some(1))
        .unwrap();
    let hover_row = app.list_area.y + 1 + (tree_index - app.list_offset) as u16;
    handle_mouse(
        &mut app,
        MouseEvent {
            kind: MouseEventKind::Moved,
            column: 5,
            row: hover_row,
            modifiers: KeyModifiers::NONE,
        },
    )?;
    assert_eq!(app.current().unwrap().id, second_id);
    handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    assert_eq!(app.screen, Screen::Detail);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn archive_requires_returning_to_list_before_confirmation() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-detail-delete-{}", Uuid::new_v4()));
    let store = configured_store(root.clone())?;
    store.start("Keep until confirmed", None, None)?;
    let mut app = App::new(store)?;
    let press = |app: &mut App, code| handle_key(app, KeyEvent::new(code, KeyModifiers::NONE));

    press(&mut app, KeyCode::Enter)?;
    press(&mut app, KeyCode::Char('a'))?;
    assert!(app.prompt.is_none());
    press(&mut app, KeyCode::Esc)?;
    assert_eq!(app.screen, Screen::List);
    assert_eq!(app.store.list()?.len(), 1);
    press(&mut app, KeyCode::Char('a'))?;
    press(&mut app, KeyCode::Enter)?;
    assert_eq!(app.screen, Screen::List);
    assert!(app.store.list()?.is_empty());
    fs::remove_dir_all(root)?;
    Ok(())
}
