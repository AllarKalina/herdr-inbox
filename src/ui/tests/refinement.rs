use super::proximity::{Fixture, coordinates, node, press, render, row};
use super::*;

#[test]
fn refine_action_opens_a_client_choice_and_cancel_preserves_the_existing_item() -> Result<()> {
    let mut fixture = Fixture::new(4)?;
    fixture.app.select_milestone(Milestone::Spec);
    let before = serde_json::to_value(fixture.app.current().unwrap())?;
    fixture.app.action_selected = fixture
        .app
        .actions()
        .iter()
        .position(|action| *action == DetailAction::RefineSpec)
        .unwrap();
    press(&mut fixture.app, KeyCode::Enter)?;
    let id = fixture.app.current().unwrap().id.clone();
    assert!(
        matches!(&fixture.app.choice_purpose, ChoicePurpose::Refine { id: target } if target == &id)
    );
    assert!(fixture.app.prompt.is_none());
    fixture.app.choose_client(
        ChoicePurpose::Refine { id: id.clone() },
        vec![Profile::Opus, Profile::Codex],
    );
    assert_eq!(fixture.app.choice_selected, Some(0));
    press(&mut fixture.app, KeyCode::Char('j'))?;
    assert_eq!(fixture.app.choice_selected, Some(1));
    let terminal = render(&mut fixture.app, 100, 35)?;
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Opus"));
    assert!(text.contains("GPT-6-Sol"));
    press(&mut fixture.app, KeyCode::Esc)?;
    assert!(fixture.app.choice_selected.is_none());
    assert!(fixture.app.prompt.is_none());
    assert!(!fixture.app.should_exit);
    assert_eq!(fixture.app.screen, Screen::Detail);
    assert_eq!(serde_json::to_value(fixture.app.store.get(&id)?)?, before);
    Ok(())
}

#[test]
fn clicking_refine_offers_clients_without_opening_an_editor_or_creating_an_item() -> Result<()> {
    let mut fixture = Fixture::new(4)?;
    fixture.app.select_milestone(Milestone::Spec);
    render(&mut fixture.app, 40, 18)?;
    let before = serde_json::to_value(fixture.app.current().unwrap())?;
    let index = fixture
        .app
        .actions()
        .iter()
        .position(|action| *action == DetailAction::RefineSpec)
        .unwrap();
    let area = fixture.app.action_hitboxes[index];
    let owner = node(&fixture.app, Milestone::Spec);
    assert!(area.x > owner.x + 7);
    handle_mouse(
        &mut fixture.app,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x + 1,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        },
        18,
    )?;
    let id = fixture.app.current().unwrap().id.clone();
    assert!(
        matches!(&fixture.app.choice_purpose, ChoicePurpose::Refine { id: target } if target == &id)
    );
    assert!(fixture.app.prompt.is_none());
    assert_eq!(fixture.app.store.list()?.len(), 1);
    assert_eq!(serde_json::to_value(fixture.app.store.get(&id)?)?, before);
    Ok(())
}

#[test]
fn unavailable_clients_keep_refinement_local_and_show_an_actionable_error() -> Result<()> {
    let mut fixture = Fixture::new(1)?;
    let id = fixture.app.current().unwrap().id.clone();
    let before = serde_json::to_value(fixture.app.current().unwrap())?;
    fixture
        .app
        .choose_client(ChoicePurpose::Refine { id: id.clone() }, vec![]);
    assert!(fixture.app.choice_selected.is_none());
    assert!(fixture.app.message.contains("No supported client"));
    assert_eq!(fixture.app.screen, Screen::Detail);
    assert_eq!(serde_json::to_value(fixture.app.store.get(&id)?)?, before);
    Ok(())
}

#[test]
fn refinement_preflight_failure_preserves_the_client_choice_for_retry() -> Result<()> {
    let mut fixture = Fixture::new(1)?;
    let before = serde_json::to_value(fixture.app.current().unwrap())?;
    let id = fixture.app.current().unwrap().id.clone();
    fs::write(&fixture.app.current().unwrap().spec_path, " \n\t")?;
    fixture.app.choose_client(
        ChoicePurpose::Refine { id: id.clone() },
        vec![Profile::Opus, Profile::Codex],
    );
    press(&mut fixture.app, KeyCode::Char('j'))?;
    let result = handle_key(
        &mut fixture.app,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    );
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Spec file is empty")
    );
    assert_eq!(fixture.app.choice_selected, Some(1));
    assert!(!fixture.app.should_exit);
    assert_eq!(fixture.app.screen, Screen::Detail);
    assert_eq!(serde_json::to_value(fixture.app.store.get(&id)?)?, before);
    press(&mut fixture.app, KeyCode::Esc)?;
    assert!(fixture.app.choice_selected.is_none());
    Ok(())
}

#[test]
fn refinement_client_choices_remain_readable_at_compact_sizes() -> Result<()> {
    for (width, height) in [(40, 18), (60, 24), (100, 35)] {
        let mut fixture = Fixture::new(1)?;
        render(&mut fixture.app, width, height)?;
        let fixed = coordinates(&fixture.app);
        let id = fixture.app.current().unwrap().id.clone();
        fixture.app.choose_client(
            ChoicePurpose::Refine { id },
            vec![Profile::Opus, Profile::Codex],
        );
        for selected in [0, 1] {
            fixture.app.choice_selected = Some(selected);
            let terminal = render(&mut fixture.app, width, height)?;
            assert_eq!(coordinates(&fixture.app), fixed);
            let text = (0..height)
                .map(|y| row(&terminal, y))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(text.contains(Profile::Opus.label()));
            assert!(text.contains(Profile::Codex.label()));
            assert!(text.contains("j/k choose"));
            for unrelated in ["PROGRESS", "JIRA ACTIONS", "j/k stage", "Bind Jira ticket"] {
                assert!(
                    !text.contains(unrelated),
                    "client choice should not expose detail controls"
                );
            }
        }
    }
    Ok(())
}
