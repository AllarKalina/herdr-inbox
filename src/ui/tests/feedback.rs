use super::support::*;
use super::*;

const SIZES: [(u16, u16); 3] = [(40, 18), (60, 24), (100, 35)];

fn complete(app: &mut App, milestone: Milestone) -> Result<()> {
    assert_eq!(app.detail.milestone, milestone);
    press(app, KeyCode::Enter)?;
    match milestone {
        Milestone::Spec => {}
        Milestone::Jira => {
            assert!(matches!(app.modal.prompt, Some(Prompt::Jira { .. })));
            app.modal.input = "PAY-123".into();
            press(app, KeyCode::Enter)?;
            assert!(matches!(app.modal.prompt, Some(Prompt::JiraUrl { .. })));
            app.modal.input = "https://jira.example/browse/PAY-123".into();
            press(app, KeyCode::Enter)?;
        }
        Milestone::Dev => {
            assert!(matches!(app.modal.prompt, Some(Prompt::Agent { .. })));
            app.modal.input = "implementor".into();
            press(app, KeyCode::Enter)?;
            assert!(matches!(app.modal.prompt, Some(Prompt::Branch { .. })));
            app.modal.input = "feature/payment-retries".into();
            press(app, KeyCode::Enter)?;
        }
        Milestone::Pr => {
            assert!(matches!(app.modal.prompt, Some(Prompt::Pr { .. })));
            app.modal.input = "https://github.example/org/repo/pull/42".into();
            press(app, KeyCode::Enter)?;
        }
    }
    Ok(())
}

#[test]
fn completed_actions_acknowledge_their_owner_and_follow_the_next_milestone() -> Result<()> {
    for (index, milestone) in Milestone::ALL.into_iter().enumerate() {
        let mut fixture = Fixture::new(index)?;
        fixture.app.notice.info("An unrelated old notice");
        complete(&mut fixture.app, milestone)?;
        assert_eq!(
            fixture.app.detail.feedback.as_ref().unwrap().milestone,
            milestone
        );
        assert!(fixture.app.notice.is_empty());
        assert!(fixture.app.modal.prompt.is_none());
        assert_eq!(
            fixture.app.detail.milestone,
            Milestone::ALL[(index + 1).min(3)]
        );
        // Polling must not prematurely remove a completion acknowledgement.
        fixture.app.refresh()?;
        fixture.app.refresh()?;
        assert_eq!(
            fixture.app.detail.feedback.as_ref().unwrap().milestone,
            milestone
        );
        press(&mut fixture.app, KeyCode::Char('k'))?;
        assert!(fixture.app.detail.feedback.is_none());
    }
    Ok(())
}

#[test]
fn finishing_an_untitled_spec_uses_the_same_local_acknowledgement() -> Result<()> {
    let mut fixture = Fixture::new(0)?;
    let record = fixture.app.store.start_untitled(None, None)?;
    fs::write(
        &record.spec_path,
        "# A named spec\n\nThe finished specification.\n",
    )?;
    press(&mut fixture.app, KeyCode::Esc)?;
    focus(&mut fixture.app, &record.id);
    fixture.open()?;
    fixture.app.select_milestone(Milestone::Spec);
    press(&mut fixture.app, KeyCode::Enter)?;
    assert!(matches!(
        fixture.app.modal.prompt,
        Some(Prompt::FinishTitle { .. })
    ));
    fixture.app.modal.input = "A named spec".into();
    press(&mut fixture.app, KeyCode::Enter)?;
    assert_eq!(fixture.app.current().unwrap().title, "A named spec");
    assert_eq!(fixture.app.current().unwrap().spec, "done");
    assert_eq!(fixture.app.detail.milestone, Milestone::Jira);
    assert_eq!(
        fixture.app.detail.feedback.as_ref().unwrap().milestone,
        Milestone::Spec
    );
    assert!(fixture.app.notice.is_empty());
    Ok(())
}

#[test]
fn updating_a_completed_stage_acknowledges_it_without_moving_on() -> Result<()> {
    for (milestone, action) in [
        (Milestone::Jira, DetailAction::UpdateJira),
        (Milestone::Dev, DetailAction::UpdateImplementation),
        (Milestone::Pr, DetailAction::UpdatePr),
    ] {
        let mut fixture = Fixture::new(4)?;
        let app = &mut fixture.app;
        app.select_milestone(milestone);
        app.detail.action = app
            .actions()
            .iter()
            .position(|candidate| *candidate == action)
            .unwrap();
        // Open the prompt, then accept each prefilled value.
        press(app, KeyCode::Enter)?;
        while app.modal.prompt.is_some() {
            press(app, KeyCode::Enter)?;
        }
        assert_eq!(app.detail.milestone, milestone);
        assert_eq!(app.detail.feedback.as_ref().unwrap().milestone, milestone);
    }
    Ok(())
}

#[test]
fn milestone_navigation_dismisses_feedback_but_action_navigation_retains_it() -> Result<()> {
    let mut fixture = Fixture::new(1)?;
    fixture.app.acknowledge(Milestone::Spec);
    press(&mut fixture.app, KeyCode::Tab)?;
    assert!(fixture.app.detail.feedback.is_some());
    press(&mut fixture.app, KeyCode::Char('k'))?;
    assert_eq!(fixture.app.detail.milestone, Milestone::Spec);
    assert!(fixture.app.detail.feedback.is_none());
    // A boundary navigation key is still an explicit dismissal.
    fixture.app.acknowledge(Milestone::Spec);
    press(&mut fixture.app, KeyCode::Char('k'))?;
    assert!(fixture.app.detail.feedback.is_none());
    fixture.app.acknowledge(Milestone::Spec);
    press(&mut fixture.app, KeyCode::Char('j'))?;
    assert!(fixture.app.detail.feedback.is_none());
    Ok(())
}

#[test]
fn clicking_and_scrolling_the_milestone_rail_dismisses_feedback() -> Result<()> {
    for (width, height) in SIZES {
        for kind in [
            MouseEventKind::Down(MouseButton::Left),
            MouseEventKind::ScrollUp,
            MouseEventKind::ScrollDown,
        ] {
            let mut fixture = Fixture::new(1)?;
            fixture.app.acknowledge(Milestone::Spec);
            render(&mut fixture.app, width, height)?;
            let owner = node(&fixture.app, Milestone::Spec);
            handle_mouse(
                &mut fixture.app,
                MouseEvent {
                    kind,
                    column: owner.x + 7,
                    row: owner.y,
                    modifiers: KeyModifiers::NONE,
                },
            )?;
            assert!(fixture.app.detail.feedback.is_none());
        }
    }
    Ok(())
}

#[test]
fn feedback_clears_when_starting_another_action_or_leaving_the_detail_view() -> Result<()> {
    let mut fixture = Fixture::new(1)?;
    fixture.app.acknowledge(Milestone::Spec);
    press(&mut fixture.app, KeyCode::Enter)?;
    assert!(matches!(
        fixture.app.modal.prompt,
        Some(Prompt::Jira { .. })
    ));
    assert!(fixture.app.detail.feedback.is_none());
    press(&mut fixture.app, KeyCode::Esc)?;
    fixture.app.acknowledge(Milestone::Spec);
    let id = fixture.app.current().unwrap().id.clone();
    fixture
        .app
        .choose_client(ChoicePurpose::Refine { id }, vec![Profile::Codex]);
    assert!(fixture.app.detail.feedback.is_none());
    press(&mut fixture.app, KeyCode::Esc)?;
    fixture.app.acknowledge(Milestone::Spec);
    press(&mut fixture.app, KeyCode::Char('r'))?;
    assert_eq!(fixture.app.screen, Screen::Reader);
    assert!(fixture.app.detail.feedback.is_none());
    press(&mut fixture.app, KeyCode::Esc)?;
    fixture.app.acknowledge(Milestone::Spec);
    press(&mut fixture.app, KeyCode::Esc)?;
    assert_eq!(fixture.app.screen, Screen::List);
    assert!(fixture.app.detail.feedback.is_none());
    let another = fixture.app.store.start("Another spec", None, None)?;
    fixture.app.refresh()?;
    focus(&mut fixture.app, &another.id);
    fixture.open()?;
    assert_eq!(fixture.app.current().unwrap().id, another.id);
    assert!(fixture.app.detail.feedback.is_none());
    Ok(())
}
