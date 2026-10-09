//! The reader, the refine choice, Settings and the archive.

use super::*;

#[test]
fn reader_and_refine_choice() -> Result<()> {
    let mut fixture = Fixture::new("reader")?;
    let long = fixture
        .app
        .records
        .iter()
        .find(|record| record.title == "Transaction search")
        .ok_or("fixture spec missing")?
        .spec_path
        .clone();
    let body: String = (1..=60).map(|n| format!("- Requirement {n}\n")).collect();
    fs::write(long, format!("# Transaction search\n\n{body}\nLAST LINE\n"))?;
    fixture.open("Transaction search")?;
    fixture.press(KeyCode::Char('r'))?;
    fixture.check("reader")?;
    for _ in 0..10 {
        fixture.press(KeyCode::Char('J'))?;
    }
    fixture.check("reader-scrolled-end")?;
    fixture.press(KeyCode::Esc)?;
    let id = fixture.app.current().unwrap().id.clone();
    fixture.app.choose_client(
        ChoicePurpose::Refine { id },
        vec![Profile::Opus, Profile::Codex],
    );
    fixture.check("refine-client-choice")?;
    fixture.finish()
}

#[test]
fn settings_and_archive() -> Result<()> {
    let mut fixture = Fixture::new("settings")?;
    fixture.focus("Inbox layout")?;
    fixture.press(KeyCode::Char('a'))?;
    fixture.press(KeyCode::Enter)?;
    fixture.app.notice.clear();
    fixture.press(KeyCode::Char('s'))?;
    fixture.check("settings-archive-one")?;
    fixture.press(KeyCode::Esc)?;
    fixture.focus("Quarterly plan")?;
    fixture.press(KeyCode::Char('a'))?;
    fixture.press(KeyCode::Enter)?;
    fixture.app.notice.clear();
    fixture.press(KeyCode::Char('s'))?;
    fixture.check("settings-folder-selected")?;
    fixture.press(KeyCode::Char('j'))?;
    fixture.press(KeyCode::Enter)?;
    fixture.check("settings-jira-turned-off")?;
    fixture.press(KeyCode::Enter)?;
    fixture.press(KeyCode::Char('j'))?;
    fixture.check("settings-archive-selected")?;
    fixture.press(KeyCode::Enter)?;
    fixture.check("archive-list")?;
    fixture.press(KeyCode::Char('j'))?;
    fixture.press(KeyCode::Char('d'))?;
    fixture.check("archive-delete-confirm")?;
    fixture.press(KeyCode::Enter)?;
    fixture.check("archive-deleted")?;
    fixture.press(KeyCode::Char('r'))?;
    fixture.check("archive-restored-empty")?;
    fixture.finish()
}
