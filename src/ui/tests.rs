use super::*;
use crossterm::event::KeyModifiers;
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
    assert!(footer.contains("d delete · q quit"));
    assert!(
        root.join("trash/items")
            .join(format!("{}.json", record.id))
            .is_file()
    );
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
        assert!(lines[1].starts_with(" Name (2)"));
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
