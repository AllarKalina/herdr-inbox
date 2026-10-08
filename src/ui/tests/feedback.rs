use super::proximity::{Fixture, coordinates, node, press, render, row};
use super::*;

const SIZES: [(u16, u16); 3] = [(40, 18), (60, 24), (100, 35)];

fn complete(app: &mut App, milestone: Milestone) -> Result<()> {
    assert_eq!(app.milestone_selected, milestone);
    press(app, KeyCode::Enter)?;
    match milestone {
        Milestone::Spec => {}
        Milestone::Jira => {
            assert!(matches!(app.prompt, Some(Prompt::Jira { .. })));
            app.input = "PAY-123".into();
            press(app, KeyCode::Enter)?;
            assert!(matches!(app.prompt, Some(Prompt::JiraUrl { .. })));
            app.input = "https://jira.example/browse/PAY-123".into();
            press(app, KeyCode::Enter)?;
        }
        Milestone::Dev => {
            assert!(matches!(app.prompt, Some(Prompt::Agent { .. })));
            app.input = "implementor".into();
            press(app, KeyCode::Enter)?;
            assert!(matches!(app.prompt, Some(Prompt::Branch { .. })));
            app.input = "feature/payment-retries".into();
            press(app, KeyCode::Enter)?;
        }
        Milestone::Pr => {
            assert!(matches!(app.prompt, Some(Prompt::Pr { .. })));
            app.input = "https://github.example/org/repo/pull/42".into();
            press(app, KeyCode::Enter)?;
        }
    }
    Ok(())
}

fn text(terminal: &Terminal<TestBackend>) -> String {
    (0..terminal.backend().buffer().area.height)
        .map(|y| row(terminal, y))
        .collect::<Vec<_>>()
        .join("\n")
}

fn feedback_text(milestone: Milestone, roomy: bool) -> &'static str {
    match (milestone, roomy) {
        (Milestone::Spec, true) => "Spec sealed",
        (Milestone::Jira, true) => "Jira bound",
        (Milestone::Dev, true) => "Dev quest logged",
        (Milestone::Pr, true) => "Draft PR bound",
        (Milestone::Spec, false) => "Sealed",
        (Milestone::Dev, false) => "Logged",
        (_, false) => "Bound",
    }
}

#[test]
fn completed_actions_acknowledge_their_owner_and_follow_the_next_milestone() -> Result<()> {
    for (index, milestone) in Milestone::ALL.into_iter().enumerate() {
        let mut fixture = Fixture::new(index)?;
        fixture.app.message = "An unrelated old notice".into();
        complete(&mut fixture.app, milestone)?;
        assert_eq!(fixture.app.feedback.as_ref().unwrap().milestone, milestone);
        assert!(fixture.app.message.is_empty());
        assert!(fixture.app.prompt.is_none());
        assert_eq!(
            fixture.app.milestone_selected,
            Milestone::ALL[(index + 1).min(3)]
        );
        // Polling must not prematurely remove a completion acknowledgement.
        fixture.app.refresh()?;
        fixture.app.refresh()?;
        assert_eq!(fixture.app.feedback.as_ref().unwrap().milestone, milestone);
        press(&mut fixture.app, KeyCode::Char('k'))?;
        assert!(fixture.app.feedback.is_none());
    }
    Ok(())
}

#[test]
fn finishing_an_untitled_spec_uses_the_same_local_acknowledgement() -> Result<()> {
    let mut fixture = Fixture::new(0)?;
    let record = fixture.app.store.start_untitled(None)?;
    fs::write(
        &record.spec_path,
        "# A named spec\n\nThe finished specification.\n",
    )?;
    fixture.app.refresh()?;
    fixture.app.selected = fixture
        .app
        .records
        .iter()
        .position(|candidate| candidate.id == record.id)
        .unwrap();
    fixture.app.select_milestone(Milestone::Spec);
    press(&mut fixture.app, KeyCode::Enter)?;
    assert!(matches!(
        fixture.app.prompt,
        Some(Prompt::FinishTitle { .. })
    ));
    fixture.app.input = "A named spec".into();
    press(&mut fixture.app, KeyCode::Enter)?;
    assert_eq!(fixture.app.current().unwrap().title, "A named spec");
    assert_eq!(fixture.app.current().unwrap().spec, "done");
    assert_eq!(fixture.app.milestone_selected, Milestone::Jira);
    assert_eq!(
        fixture.app.feedback.as_ref().unwrap().milestone,
        Milestone::Spec
    );
    assert!(fixture.app.message.is_empty());
    Ok(())
}

#[test]
fn feedback_stays_beside_its_owner_without_moving_nodes_or_action_slots() -> Result<()> {
    for (width, height) in SIZES {
        for (index, milestone) in Milestone::ALL.into_iter().enumerate() {
            let mut fixture = Fixture::new(index)?;
            render(&mut fixture.app, width, height)?;
            let fixed_nodes = coordinates(&fixture.app);
            complete(&mut fixture.app, milestone)?;
            let terminal = render(&mut fixture.app, width, height)?;
            assert_eq!(coordinates(&fixture.app), fixed_nodes);
            let fixed_actions = fixture.app.action_hitboxes.clone();
            let owner = node(&fixture.app, milestone);
            let expected = feedback_text(milestone, owner.width > 18);
            let x = owner.x + 11;
            let y = owner.y + 1;
            let buffer = terminal.backend().buffer();
            for (offset, character) in expected.chars().enumerate() {
                let cell = &buffer[(x + offset as u16, y)];
                assert_eq!(cell.symbol(), character.to_string());
                assert_eq!(cell.fg, Color::LightGreen);
                assert_eq!(cell.bg, Color::Reset);
            }
            let footer_start = (0..height)
                .find(|&y| row(&terminal, y).contains("j/k stage"))
                .expect("detail navigation footer must remain visible");
            let footer = (footer_start..height)
                .map(|y| row(&terminal, y))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(footer.contains("j/k stage"));
            assert!(!footer.contains(expected));
            fixture.app.feedback = None;
            render(&mut fixture.app, width, height)?;
            assert_eq!(coordinates(&fixture.app), fixed_nodes);
            assert_eq!(fixture.app.action_hitboxes, fixed_actions);
        }
    }
    Ok(())
}

#[test]
fn updating_completed_stages_keeps_compact_feedback_in_the_owner_caption() -> Result<()> {
    for (width, height) in [(40, 18), (60, 24)] {
        for milestone in [Milestone::Jira, Milestone::Dev, Milestone::Pr] {
            let mut fixture = Fixture::new(4)?;
            fixture.app.select_milestone(milestone);
            let action = match milestone {
                Milestone::Jira => DetailAction::UpdateJira,
                Milestone::Dev => DetailAction::UpdateImplementation,
                Milestone::Pr => DetailAction::UpdatePr,
                _ => unreachable!(),
            };
            fixture.app.action_selected = fixture
                .app
                .actions()
                .iter()
                .position(|candidate| *candidate == action)
                .unwrap();
            press(&mut fixture.app, KeyCode::Enter)?;
            press(&mut fixture.app, KeyCode::Enter)?;
            if milestone != Milestone::Pr {
                press(&mut fixture.app, KeyCode::Enter)?;
            }
            assert_eq!(fixture.app.milestone_selected, milestone);
            assert_eq!(fixture.app.feedback.as_ref().unwrap().milestone, milestone);
            let terminal = render(&mut fixture.app, width, height)?;
            let owner = node(&fixture.app, milestone);
            let expected = feedback_text(milestone, false);
            let caption: String = (owner.x + 11..owner.x + 18)
                .map(|x| terminal.backend().buffer()[(x, owner.y + 1)].symbol())
                .collect();
            assert_eq!(caption.trim(), expected);
            assert!(!text(&terminal).contains(feedback_text(milestone, true)));
        }
    }
    Ok(())
}

#[test]
fn milestone_navigation_dismisses_feedback_but_action_navigation_retains_it() -> Result<()> {
    let mut fixture = Fixture::new(1)?;
    fixture.app.acknowledge(Milestone::Spec);
    press(&mut fixture.app, KeyCode::Tab)?;
    assert!(fixture.app.feedback.is_some());
    press(&mut fixture.app, KeyCode::Char('k'))?;
    assert_eq!(fixture.app.milestone_selected, Milestone::Spec);
    assert!(fixture.app.feedback.is_none());
    // A boundary navigation key is still an explicit dismissal.
    fixture.app.acknowledge(Milestone::Spec);
    press(&mut fixture.app, KeyCode::Char('k'))?;
    assert!(fixture.app.feedback.is_none());
    fixture.app.acknowledge(Milestone::Spec);
    press(&mut fixture.app, KeyCode::Char('j'))?;
    assert!(fixture.app.feedback.is_none());
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
            assert!(fixture.app.feedback.is_none());
        }
    }
    Ok(())
}

#[test]
fn feedback_clears_when_starting_another_action_or_leaving_the_detail_view() -> Result<()> {
    let mut fixture = Fixture::new(1)?;
    fixture.app.acknowledge(Milestone::Spec);
    press(&mut fixture.app, KeyCode::Enter)?;
    assert!(matches!(fixture.app.prompt, Some(Prompt::Jira { .. })));
    assert!(fixture.app.feedback.is_none());
    press(&mut fixture.app, KeyCode::Esc)?;
    fixture.app.acknowledge(Milestone::Spec);
    let id = fixture.app.current().unwrap().id.clone();
    fixture
        .app
        .choose_client(ChoicePurpose::Refine { id }, vec![Profile::Codex]);
    assert!(fixture.app.feedback.is_none());
    press(&mut fixture.app, KeyCode::Esc)?;
    fixture.app.acknowledge(Milestone::Spec);
    press(&mut fixture.app, KeyCode::Char('r'))?;
    assert_eq!(fixture.app.screen, Screen::Reader);
    assert!(fixture.app.feedback.is_none());
    press(&mut fixture.app, KeyCode::Esc)?;
    fixture.app.acknowledge(Milestone::Spec);
    press(&mut fixture.app, KeyCode::Esc)?;
    assert_eq!(fixture.app.screen, Screen::List);
    assert!(fixture.app.feedback.is_none());
    let another = fixture.app.store.start("Another spec", None, None)?;
    fixture.app.refresh()?;
    fixture.app.selected = fixture
        .app
        .records
        .iter()
        .position(|record| record.id == another.id)
        .unwrap();
    fixture.app.tree.focus_record(fixture.app.selected);
    press(&mut fixture.app, KeyCode::Enter)?;
    assert_eq!(fixture.app.current().unwrap().id, another.id);
    assert!(fixture.app.feedback.is_none());
    Ok(())
}
