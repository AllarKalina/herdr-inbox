use super::*;

fn rendered(app: &mut App, width: u16, height: u16) -> Result<Vec<String>> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| draw::draw(frame, app))?;
    Ok(terminal
        .backend()
        .buffer()
        .content()
        .chunks(width as usize)
        .map(|cells| cells.iter().map(|cell| cell.symbol()).collect())
        .collect())
}

#[test]
fn progress_timeline_keeps_linked_evidence_visible() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-evidence-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    let record = store.start("Payment retry handling", None, None)?;
    store.update(&record.id, Change::Finish { title: None })?;
    store.update(
        &record.id,
        Change::Jira {
            key: "PAY-123".into(),
            url: Some("https://jira.example/browse/PAY-123".into()),
        },
    )?;
    store.update(
        &record.id,
        Change::Implement {
            agent: Some("implementor".into()),
            branch: Some("feature/payment-retries".into()),
        },
    )?;
    store.update(
        &record.id,
        Change::Pr {
            url: "https://github.example/org/repo/pull/42".into(),
        },
    )?;
    let mut app = App::new(store)?;
    handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    let lines = rendered(&mut app, 100, 35)?;
    assert!(lines.iter().any(|line| line.contains("PAY-123")));
    assert!(lines.iter().any(|line| line.contains("DEV    ●   done")));
    assert!(
        lines
            .iter()
            .any(|line| line.contains("feature/payment-retries"))
    );
    assert!(lines.iter().any(|line| line.contains("#42 ↗")));
    assert!(lines.iter().any(|line| line.contains("Review draft PR")));
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn narrow_detail_keeps_progress_and_actions_visible() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-narrow-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    store.start(
        "A very long title that should not obscure the next action",
        None,
        None,
    )?;
    let mut app = App::new(store)?;
    handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    let lines = rendered(&mut app, 60, 24)?;
    assert!(lines.iter().any(|line| line.contains("PROGRESS")));
    assert!(lines.iter().any(|line| line.contains("DEV    ○   locked")));
    assert!(lines.iter().any(|line| line.contains("SPEC ACTIONS")));
    assert!(!lines.iter().any(|line| line.contains("NEXT MOVE")));
    assert!(lines.iter().any(|line| line.contains("Seal the spec")));
    assert!(
        lines
            .iter()
            .any(|line| line.contains("j/k stage · Tab action · Enter act"))
    );
    let button = app.action_hitboxes[0];
    assert!(button.x + button.width <= 60);
    handle_mouse(
        &mut app,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: button.x + 2,
            row: button.y,
            modifiers: KeyModifiers::NONE,
        },
        24,
    )?;
    assert_eq!(app.current().unwrap().spec, "done");
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn reader_keeps_final_crumb_after_long_title() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-crumb-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    store.start(
        "A long spec title that would otherwise occupy the entire breadcrumb width",
        None,
        None,
    )?;
    let mut app = App::new(store)?;
    handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
    )?;
    let lines = rendered(&mut app, 42, 18)?;
    assert!(lines.iter().any(|line| line.contains("FULL SPEC")));
    assert!(lines.iter().any(|line| line.contains('…')));
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn compact_list_keeps_title_and_four_waypoint_trail() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-small-list-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    store.start("A still readable spec title", None, None)?;
    let mut app = App::new(store)?;
    let lines = rendered(&mut app, 40, 18)?;
    assert!(lines.iter().any(|line| line.contains("S J D P")));
    assert!(
        lines
            .iter()
            .any(|line| line.contains("A still readable spec"))
    );
    assert!(lines.iter().any(|line| line.contains("● ○ ○ ○")));
    fs::remove_dir_all(root)?;
    Ok(())
}
