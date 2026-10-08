use super::*;

#[test]
fn list_keeps_status_columns_fixed_after_long_names() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-list-{}", Uuid::new_v4()));
    let store = configured_store(root.clone())?;
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
        let header = lines
            .iter()
            .find(|line| {
                line.contains("S J D P") || (line.contains("Spec") && line.contains("Jira"))
            })
            .unwrap();
        if width < 64 {
            assert!(header.contains("S J D P"));
            assert!(rows.iter().all(|row| row.contains("● ○ ○ ○")));
        } else {
            assert!(header.contains("Spec") && header.contains("Jira"));
            let columns: Vec<_> = lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.contains("Short") || line.contains("Long title"))
                .map(|(y, _)| {
                    (0..width)
                        .find(|&x| terminal.backend().buffer()[(x, y as u16)].symbol() == "●")
                        .unwrap()
                })
                .collect();
            assert_eq!(columns[0], columns[1]);
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
    let store = configured_store(root.clone())?;
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
