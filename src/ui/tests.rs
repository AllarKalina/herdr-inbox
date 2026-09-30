use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use std::fs;
use uuid::Uuid;

#[test]
fn delete_requires_second_enter_and_esc_cancels() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-ui-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    let record = store.start("Keep until confirmed", None, None)?;
    let mut app = App::new(store)?;
    let press = |app: &mut App, code| handle_key(app, KeyEvent::new(code, KeyModifiers::NONE));

    press(&mut app, KeyCode::Char('d'))?;
    assert!(matches!(app.prompt, Some(Prompt::Delete { .. })));
    press(&mut app, KeyCode::Esc)?;
    assert!(app.store.get(&record.id).is_ok());

    press(&mut app, KeyCode::Char('d'))?;
    press(&mut app, KeyCode::Char('x'))?;
    assert!(app.store.get(&record.id).is_ok());
    assert!(app.input.is_empty());
    press(&mut app, KeyCode::Enter)?;
    assert!(app.store.list()?.is_empty());
    let footer = draw::footer_text(&app);
    assert!(footer.starts_with("Item moved to local Trash\n"));
    assert!(footer.contains("Enter open · n new · d delete"));
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
    let store = Store::new(root.clone());
    let record = store.start("Keep in progress", None, None)?;
    let mut app = App::new(store)?;

    for key in ['a', 'f', 't', 'J', 'i', 'p', 'q'] {
        assert!(!handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE)
        )?);
        assert!(app.prompt.is_none());
    }
    assert_eq!(app.store.get(&record.id)?.spec, "in_progress");
    assert_eq!(draw::footer_text(&app), "Enter open · n new · d delete");
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn list_keeps_status_columns_fixed_after_long_names() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-list-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    store.start("Short", None, None)?;
    store.start(
        "Long title that stretches well beyond the available name column width",
        None,
        None,
    )?;
    let mut app = App::new(store)?;

    for width in [54, 100] {
        let mut terminal = Terminal::new(TestBackend::new(width, 24))?;
        terminal.draw(|frame| draw::draw(frame, &mut app))?;
        let lines: Vec<String> = terminal
            .backend()
            .buffer()
            .content()
            .chunks(width as usize)
            .map(|cells| cells.iter().map(|cell| cell.symbol()).collect())
            .collect();
        assert!(lines[0].trim().is_empty());
        assert!(!lines[1].contains("Name"));
        assert!(lines[1].contains("Spec") && lines[1].contains("Jira"));
        let rows: Vec<&String> = lines
            .iter()
            .filter(|line| line.contains("● active"))
            .collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].find("● active"), rows[1].find("● active"));
        assert!(
            rows.iter()
                .all(|row| row.rfind("○ wait").unwrap() > width as usize - 15)
        );
        assert!(rows.iter().any(|row| row.contains("Short")));
        assert!(rows.iter().any(|row| row.contains("Long title")));
        let active_colors: Vec<Color> = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .filter(|cell| cell.symbol() == "●")
            .map(|cell| cell.fg)
            .collect();
        assert!(active_colors.contains(&Color::Black));
        assert!(active_colors.contains(&Color::Cyan));
    }
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn list_icons_show_completed_and_draft_stages() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-icons-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    let record = store.start("Completed item", None, None)?;
    store.update(&record.id, Change::Finish { title: None })?;
    store.update(
        &record.id,
        Change::Jira {
            key: "TEST-1".into(),
            url: None,
        },
    )?;
    store.update(
        &record.id,
        Change::Pr {
            url: "https://example.test/pr/1".into(),
        },
    )?;
    let mut app = App::new(store)?;
    let mut terminal = Terminal::new(TestBackend::new(100, 24))?;
    terminal.draw(|frame| draw::draw(frame, &mut app))?;
    let lines: Vec<String> = terminal
        .backend()
        .buffer()
        .content()
        .chunks(100)
        .map(|cells| cells.iter().map(|cell| cell.symbol()).collect())
        .collect();
    let row = lines
        .iter()
        .find(|line| line.contains("Completed item"))
        .unwrap();
    assert_eq!(row.matches("✓ done").count(), 2);
    assert_eq!(row.matches("◐ draft").count(), 2);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn selected_spec_opens_detail_and_full_reader_then_returns() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-detail-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
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
    assert!(!rendered.contains("v read"));
    assert!(rendered.contains("Retry only transient failures"));
    assert!(rendered.contains("QUEST PATH"));
    assert!(rendered.contains("Finish spec"));
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
    let quest_column = lines
        .iter()
        .find_map(|line| line.find("QUEST PATH"))
        .unwrap();
    assert_eq!(heading_column, content_column);
    assert_eq!(heading_column, quest_column);

    press(&mut app, KeyCode::Char('v'))?;
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
    assert!(rendered.contains("Retry only transient failures"));
    press(&mut app, KeyCode::Esc)?;
    assert_eq!(app.screen, Screen::Detail);
    press(&mut app, KeyCode::Esc)?;
    assert_eq!(app.screen, Screen::List);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn quest_actions_follow_the_real_parallel_stages() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-quest-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    store.start("Payment retries", None, None)?;
    let mut app = App::new(store)?;
    let press = |app: &mut App, code| handle_key(app, KeyEvent::new(code, KeyModifiers::NONE));

    press(&mut app, KeyCode::Enter)?;
    assert_eq!(
        detail_actions(app.current().unwrap()),
        vec![DetailAction::Finish]
    );
    press(&mut app, KeyCode::Enter)?;
    assert_eq!(app.current().unwrap().spec, "done");
    assert_eq!(
        detail_actions(app.current().unwrap()),
        vec![DetailAction::Jira, DetailAction::Implement]
    );
    press(&mut app, KeyCode::Tab)?;
    press(&mut app, KeyCode::Enter)?;
    assert!(matches!(app.prompt, Some(Prompt::Agent { .. })));
    press(&mut app, KeyCode::Enter)?;
    assert!(matches!(app.prompt, Some(Prompt::Branch { .. })));
    press(&mut app, KeyCode::Enter)?;
    assert_eq!(app.current().unwrap().implementation.status, "in_progress");
    assert_eq!(
        detail_actions(app.current().unwrap()),
        vec![DetailAction::Jira, DetailAction::Pr]
    );
    handle_mouse(
        &mut app,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 15,
            row: 30,
            modifiers: KeyModifiers::NONE,
        },
        35,
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
    let id = app.current().unwrap().id.clone();
    app.store.update(
        &id,
        Change::Pr {
            url: "https://github.example/pr/1".into(),
        },
    )?;
    app.refresh()?;
    assert_eq!(
        detail_actions(app.current().unwrap()),
        vec![DetailAction::ReviewPr]
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn mouse_hover_selects_a_list_item_before_enter() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-mouse-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    store.start("First", None, None)?;
    store.start("Second", None, None)?;
    let mut app = App::new(store)?;
    let second_id = app.records[1].id.clone();
    handle_mouse(
        &mut app,
        MouseEvent {
            kind: MouseEventKind::Moved,
            column: 5,
            row: 3,
            modifiers: KeyModifiers::NONE,
        },
        30,
    )?;
    assert_eq!(app.current().unwrap().id, second_id);
    handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    assert_eq!(app.screen, Screen::Detail);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn detail_delete_still_requires_confirmation_and_returns_to_list() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-detail-delete-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    store.start("Keep until confirmed", None, None)?;
    let mut app = App::new(store)?;
    let press = |app: &mut App, code| handle_key(app, KeyEvent::new(code, KeyModifiers::NONE));

    press(&mut app, KeyCode::Enter)?;
    press(&mut app, KeyCode::Char('d'))?;
    assert!(matches!(app.prompt, Some(Prompt::Delete { .. })));
    press(&mut app, KeyCode::Esc)?;
    assert_eq!(app.screen, Screen::Detail);
    assert_eq!(app.store.list()?.len(), 1);
    press(&mut app, KeyCode::Char('d'))?;
    press(&mut app, KeyCode::Enter)?;
    assert_eq!(app.screen, Screen::List);
    assert!(app.store.list()?.is_empty());
    fs::remove_dir_all(root)?;
    Ok(())
}
