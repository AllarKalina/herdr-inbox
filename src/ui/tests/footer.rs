use super::*;

fn press(app: &mut App, code: KeyCode) -> Result<bool> {
    handle_key(app, KeyEvent::new(code, KeyModifiers::NONE))
}

#[test]
fn minimal_footer_is_one_line_and_setup_shortcuts_only_work_in_settings() -> Result<()> {
    let root = std::env::temp_dir().join(format!("inbox-minimal-menu-{}", Uuid::new_v4()));
    let store = configured_store(root.join("data"))?;
    let record = store.start("Existing item", None, None)?;
    let mut app = App::new(store)?;
    assert!(app.message.is_empty());
    assert_eq!(
        draw::footer_text(&app, 100),
        "Enter open · n new · a archive · s settings"
    );
    for (width, height) in [(40, 18), (100, 35)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        terminal.draw(|frame| draw::draw(frame, &mut app))?;
        let lines: Vec<String> = (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                    .collect()
            })
            .collect();
        let menu: Vec<_> = lines
            .iter()
            .filter(|line| line.contains("a archive"))
            .collect();
        assert_eq!(menu.len(), 1);
        assert!(menu[0].starts_with("  Enter") || menu[0].starts_with("  ↵"));
        assert_eq!(menu[0].chars().position(|ch| !ch.is_whitespace()), Some(2));
        assert!(menu[0].contains("s settings"));
        assert!(menu[0].contains("n new") || menu[0].contains("· n ·"));
        assert!(
            menu[0].ends_with("  "),
            "right inset must remain visible: {}",
            menu[0]
        );
        assert!(!lines.iter().any(|line| line.contains("connect your specs")
            || line.contains("S scan")
            || line.contains("u restore")));
        fs::write(
            root.join(format!("footer-{width}x{height}.txt")),
            lines.join("\n"),
        )?;
        save_snapshot(
            terminal.backend().buffer(),
            &lines,
            &format!("footer-list-{width}x{height}"),
        )?;
    }
    for key in ['d', 'S', 'u'] {
        press(&mut app, KeyCode::Char(key))?;
        assert_eq!(app.screen, Screen::List);
        assert!(app.prompt.is_none());
    }
    assert!(app.store.get(&record.id).is_ok());
    let specs = root.join("chosen folder");
    fs::create_dir(&specs)?;
    let mut settings = app.store.settings()?;
    settings
        .sources
        .push(crate::store::SpecSource::new(specs.clone())?);
    app.store.save_settings(&settings)?;
    fs::write(specs.join("new.md"), "# New item\n")?;
    press(&mut app, KeyCode::Char('S'))?;
    assert_eq!(app.records.len(), 1);
    press(&mut app, KeyCode::Char('s'))?;
    assert_eq!(app.screen, Screen::Settings);
    press(&mut app, KeyCode::Char('s'))?;
    assert_eq!(app.records.len(), 2);
    fs::remove_dir_all(root)?;
    Ok(())
}

fn save_snapshot(buffer: &ratatui::buffer::Buffer, lines: &[String], name: &str) -> Result<()> {
    if let Some(directory) = std::env::var_os("HERDR_INBOX_UI_SNAPSHOTS") {
        let directory = PathBuf::from(directory);
        fs::create_dir_all(&directory)?;
        fs::write(directory.join(format!("{name}.txt")), lines.join("\n"))?;
        let cells: Vec<Vec<_>> = buffer.content().chunks(buffer.area.width as usize)
            .map(|row| row.iter().map(|cell| serde_json::json!({
                "symbol": cell.symbol(), "fg": format!("{:?}", cell.fg),
                "bg": format!("{:?}", cell.bg), "bold": cell.modifier.contains(ratatui::style::Modifier::BOLD)
            })).collect()).collect();
        fs::write(
            directory.join(format!("{name}.json")),
            serde_json::to_vec(&cells)?,
        )?;
    }
    Ok(())
}

#[test]
fn spec_navigation_keeps_shared_inset_without_relink_edit_or_archive_shortcuts() -> Result<()> {
    let root = std::env::temp_dir().join(format!("inbox-spec-nav-{}", Uuid::new_v4()));
    let store = configured_store(root.join("data"))?;
    store.start("Mock spec", None, None)?;
    let mut app = App::new(store)?;
    for screen in [Screen::Detail, Screen::Reader] {
        app.screen = screen;
        for (width, height) in [(40, 18), (100, 35)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height))?;
            terminal.draw(|frame| draw::draw(frame, &mut app))?;
            let lines: Vec<String> = terminal
                .backend()
                .buffer()
                .content()
                .chunks(width as usize)
                .map(|row| row.iter().map(|cell| cell.symbol()).collect())
                .collect();
            let navigation = lines.iter().find(|line| line.contains("j/k")).unwrap();
            assert_eq!(
                navigation.chars().position(|ch| !ch.is_whitespace()),
                Some(2)
            );
            let rendered = lines.join("\n");
            for removed in ["L relink", "e edit", "a archive"] {
                assert!(
                    !rendered.contains(removed),
                    "{screen:?} still displays {removed}"
                );
            }
            save_snapshot(
                terminal.backend().buffer(),
                &lines,
                &format!("footer-{screen:?}-{width}x{height}"),
            )?;
        }
    }
    fs::remove_dir_all(root)?;
    Ok(())
}
