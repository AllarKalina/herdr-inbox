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
        draw::footer_text(&app),
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
        assert!(menu[0].contains("s settings"));
        assert!(!lines.iter().any(|line| line.contains("connect your specs")
            || line.contains("S scan")
            || line.contains("u restore")));
        fs::write(
            root.join(format!("footer-{width}x{height}.txt")),
            lines.join("\n"),
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
