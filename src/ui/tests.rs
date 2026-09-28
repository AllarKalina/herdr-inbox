use super::*;
use crossterm::event::KeyModifiers;
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
