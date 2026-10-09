//! The detail view: every stage, every selectable milestone, every prompt and acknowledgement.

use super::*;

#[test]
fn detail_at_every_stage() -> Result<()> {
    let mut fixture = Fixture::new("detail")?;
    for (title, name) in [
        ("Settings navigation", "detail-spec-active"),
        ("Inbox layout", "detail-jira-ready"),
        ("Transaction search", "detail-dev-ready"),
        ("Settlement reconciliation", "detail-pr-ready"),
        ("Quarterly plan", "detail-pr-draft"),
    ] {
        fixture.open(title)?;
        fixture.check(name)?;
    }
    fixture.finish()
}

#[test]
fn every_milestone_can_be_inspected() -> Result<()> {
    let mut fixture = Fixture::new("detail-inspect")?;
    // Stages that are not yet reachable explain what they wait for.
    fixture.open("Settings navigation")?;
    fixture.press(KeyCode::Char('j'))?;
    fixture.check("detail-jira-wait-selected")?;
    fixture.open("Inbox layout")?;
    fixture.press(KeyCode::Char('j'))?;
    fixture.check("detail-locked-stage-selected")?;
    fixture.press(KeyCode::Char('j'))?;
    fixture.check("detail-locked-pr-selected")?;
    // Completed stages keep their own actions.
    fixture.open("Quarterly plan")?;
    for name in [
        "detail-done-dev-selected",
        "detail-done-jira-selected",
        "detail-done-spec-selected",
    ] {
        fixture.press(KeyCode::Char('k'))?;
        fixture.check(name)?;
    }
    fixture.app.notice.success("Opened Jira ticket");
    fixture.check("detail-status-message")?;
    fixture.finish()
}

#[test]
fn prompts_and_acknowledgements() -> Result<()> {
    let mut fixture = Fixture::new("detail-prompts")?;
    fixture.open("Settings navigation")?;
    fixture.press(KeyCode::Char('x'))?;
    fixture.check("detail-settle-confirm")?;
    fixture.press(KeyCode::Esc)?;
    fixture.app.notice.clear();
    fixture.press(KeyCode::Enter)?;
    fixture.check("detail-spec-sealed-feedback")?;

    // The first Jira action asks which epic or story the new ticket belongs under.
    fixture.open("Inbox layout")?;
    fixture.press(KeyCode::Enter)?;
    fixture.type_text("PAY-100")?;
    fixture.check("detail-jira-parent-prompt")?;
    fixture.press(KeyCode::Esc)?;
    let id = fixture.app.current().unwrap().id.clone();
    fixture.app.store.update(&id, Change::RequestJira)?;
    fixture.app.refresh()?;
    fixture.check("detail-jira-requested")?;
    // Linking an existing ticket by hand is the second action.
    fixture.press(KeyCode::Tab)?;
    fixture.press(KeyCode::Enter)?;
    fixture.type_text("PAY-9")?;
    fixture.check("detail-jira-key-prompt")?;
    fixture.press(KeyCode::Enter)?;
    fixture.type_text("https://jira.example/browse/PAY-9")?;
    fixture.check("detail-jira-url-prompt")?;
    fixture.press(KeyCode::Enter)?;
    fixture.check("detail-jira-bound-feedback")?;

    // With a development skill configured, starting it is the first Dev action.
    fixture.open("Transaction search")?;
    fixture.app.store.update_settings(|settings| {
        settings.dev_skill = Some("/team-dev".into());
        Ok(())
    })?;
    fixture.app.refresh()?;
    fixture.check("detail-dev-skill-configured")?;
    fixture.press(KeyCode::Tab)?;
    fixture.press(KeyCode::Enter)?;
    fixture.type_text("codex")?;
    fixture.check("detail-dev-agent-prompt")?;
    fixture.press(KeyCode::Enter)?;
    fixture.type_text("feature/search")?;
    fixture.check("detail-dev-branch-prompt")?;
    fixture.press(KeyCode::Enter)?;
    fixture.check("detail-dev-logged-feedback")?;

    fixture.open("Settlement reconciliation")?;
    fixture.press(KeyCode::Enter)?;
    fixture.type_text("https://github.example/org/repo/pull/7")?;
    fixture.check("detail-pr-url-prompt")?;
    fixture.type_text(&"/files/a-very-long-path-segment".repeat(5))?;
    fixture.check("detail-prompt-long-input")?;
    fixture.press(KeyCode::Enter)?;
    fixture.check("detail-pr-bound-feedback")?;

    // Updating a finished stage acknowledges it without moving on.
    fixture.open("Quarterly plan")?;
    fixture.press(KeyCode::Char('k'))?;
    fixture.press(KeyCode::Char('k'))?;
    fixture.press(KeyCode::Tab)?;
    for _ in 0..3 {
        fixture.press(KeyCode::Enter)?;
    }
    fixture.check("detail-jira-updated-feedback")?;
    fixture.finish()
}

#[test]
fn untitled_spec_asks_for_its_title() -> Result<()> {
    let mut fixture = Fixture::new("detail-untitled")?;
    let file = fixture.root.join("specs/untitled-draft.md");
    let record = fixture.app.store.start_untitled(None, Some(file.clone()))?;
    fs::write(file, "Notes without a heading yet.\n")?;
    fixture.app.refresh()?;
    let index = fixture
        .app
        .records
        .iter()
        .position(|shown| shown.id == record.id)
        .ok_or("untitled spec missing")?;
    fixture.app.list.tree.focus_record(index);
    fixture.press(KeyCode::Enter)?;
    fixture.check("detail-untitled")?;
    fixture.press(KeyCode::Enter)?;
    fixture.type_text("Draft")?;
    fixture.check("detail-finish-title-prompt")?;
    fixture.finish()
}

#[test]
fn detail_without_jira() -> Result<()> {
    let mut fixture = Fixture::new("detail-jira-off")?;
    fixture.set_jira(false)?;
    for (title, name) in [
        ("Settings navigation", "detail-jira-off-spec-active"),
        ("Inbox layout", "detail-jira-off-dev-ready"),
        ("Quarterly plan", "detail-jira-off-pr-draft"),
    ] {
        fixture.open(title)?;
        fixture.check(name)?;
    }
    fixture.finish()
}
