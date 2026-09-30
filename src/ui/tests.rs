use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use std::fs;
use uuid::Uuid;

mod quest;
mod timeline;

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
        let rows: Vec<&String> = lines
            .iter()
            .filter(|line| line.contains("Short") || line.contains("Long title"))
            .collect();
        assert_eq!(rows.len(), 2);
        if width < 64 {
            assert!(lines[1].contains("S J D P"));
            assert!(rows.iter().all(|row| row.contains("● ○ ○ ○")));
        } else {
            assert!(lines[1].contains("Spec") && lines[1].contains("Jira"));
            assert_eq!(rows[0].find("● active"), rows[1].find("● active"));
            assert!(
                rows.iter()
                    .all(|row| row.rfind("○ locked").unwrap() > width as usize - 15)
            );
        }
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
        if width >= 64 {
            assert!(active_colors.contains(&Color::Black));
            assert!(active_colors.contains(&Color::Cyan));
        }
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
        Change::Implement {
            agent: None,
            branch: None,
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
    assert_eq!(row.matches("✓ done").count(), 3);
    assert_eq!(row.matches("◐ draft").count(), 1);
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
    assert!(!rendered.contains("v full spec"));
    assert!(rendered.contains("Retry only transient failures"));
    assert!(rendered.contains("PROGRESS"));
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
    let progress_column = lines.iter().find_map(|line| line.find("PROGRESS")).unwrap();
    assert_eq!(heading_column, content_column);
    assert!(progress_column > content_column + 30);
    let milestone_rows: Vec<usize> = [
        "SPEC   ◉ active",
        "JIRA   ○ wait",
        "DEV    ○ locked",
        "PR     ○ locked",
    ]
    .iter()
    .map(|milestone| {
        lines
            .iter()
            .position(|line| line.contains(milestone))
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
    assert!(rendered.contains("← Inbox  /  Payment retries  /  FULL SPEC"));
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
    let store = Store::new(root.clone());
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
        16,
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
    let store = Store::new(root.clone());
    store.start("Payment retries", None, None)?;
    let mut app = App::new(store)?;
    let press = |app: &mut App, code| handle_key(app, KeyEvent::new(code, KeyModifiers::NONE));

    press(&mut app, KeyCode::Enter)?;
    assert_eq!(
        app.actions(),
        vec![
            DetailAction::Finish,
            DetailAction::ReadSpec,
            DetailAction::EditSpec
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
