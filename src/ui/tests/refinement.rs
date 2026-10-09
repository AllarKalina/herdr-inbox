//! The client choice that starts a refinement session.

use super::support::*;
use super::*;

fn selected(app: &App) -> Option<usize> {
    app.modal.choice.as_ref().map(|choice| choice.selected)
}

#[test]
fn refine_action_opens_a_client_choice_and_cancel_preserves_the_existing_item() -> Result<()> {
    let mut fixture = Fixture::new(4)?;
    let app = &mut fixture.app;
    let before = serde_json::to_value(app.current().unwrap())?;
    app.select_milestone(Milestone::Spec);
    app.detail.action = app
        .actions()
        .iter()
        .position(|action| *action == DetailAction::RefineSpec)
        .unwrap();
    // No client is installed in the test environment, so offer two explicitly.
    app.choose_client(
        ChoicePurpose::Refine {
            id: fixture.id.clone(),
        },
        vec![Profile::Opus, Profile::Codex],
    );
    let choice = app.modal.choice.as_ref().unwrap();
    assert_eq!(
        choice.purpose,
        ChoicePurpose::Refine {
            id: fixture.id.clone()
        }
    );
    assert_eq!(selected(app), Some(0));
    press(app, KeyCode::Char('j'))?;
    press(app, KeyCode::Char('j'))?;
    assert_eq!(selected(app), Some(1), "selection stops at the last client");
    press(app, KeyCode::Esc)?;
    assert!(!app.modal.is_open());
    assert!(!app.should_exit);
    assert_eq!(app.screen, Screen::Detail);
    assert_eq!(serde_json::to_value(app.store.get(&fixture.id)?)?, before);
    Ok(())
}

#[test]
fn unavailable_clients_keep_refinement_local_and_show_an_actionable_error() -> Result<()> {
    let mut fixture = Fixture::new(1)?;
    let app = &mut fixture.app;
    let before = serde_json::to_value(app.current().unwrap())?;
    let id = fixture.id.clone();
    app.choose_client(ChoicePurpose::Refine { id }, Vec::new());
    assert!(!app.modal.is_open());
    assert!(app.notice.text().contains("No supported client"));
    assert_eq!(app.screen, Screen::Detail);
    assert_eq!(serde_json::to_value(app.store.get(&fixture.id)?)?, before);
    Ok(())
}

#[test]
fn refinement_preflight_failure_preserves_the_client_choice_for_retry() -> Result<()> {
    let mut fixture = Fixture::new(1)?;
    let app = &mut fixture.app;
    let before = serde_json::to_value(app.current().unwrap())?;
    fs::write(&app.current().unwrap().spec_path, " \n\t")?;
    let id = fixture.id.clone();
    app.choose_client(
        ChoicePurpose::Refine { id },
        vec![Profile::Opus, Profile::Codex],
    );
    press(app, KeyCode::Char('j'))?;
    press(app, KeyCode::Enter)?;
    assert!(app.notice.text().contains("Spec file is empty"));
    assert_eq!(
        selected(app),
        Some(1),
        "the choice stays open for another try"
    );
    assert!(!app.should_exit);
    assert_eq!(app.screen, Screen::Detail);
    assert_eq!(serde_json::to_value(app.store.get(&fixture.id)?)?, before);
    Ok(())
}
