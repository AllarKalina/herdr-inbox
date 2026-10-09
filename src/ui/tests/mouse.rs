//! Mouse hit-testing in the detail view. Snapshots pin what is drawn; these pin that the
//! clickable areas are where the drawing is, and what a click does.

use super::support::*;
use super::*;

/// The text drawn inside a hitbox, with wrapped lines joined.
fn drawn(terminal: &Terminal<TestBackend>, area: Rect) -> String {
    let buffer = terminal.backend().buffer();
    (area.y..area.bottom())
        .map(|y| {
            (area.x..buffer.area.width - 2)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn clicking_a_milestone_label_or_its_node_area_selects_it_without_acting() -> Result<()> {
    let mut fixture = Fixture::new(4)?;
    let record = fixture.root.join(format!("items/{}.json", fixture.id));
    let before = fs::read(&record)?;
    for (width, height) in [(40, 18), (100, 35)] {
        render(&mut fixture.app, width, height)?;
        for stage in Milestone::ALL {
            let area = node(&fixture.app, stage);
            // The label row, then the four corners of the five-by-three node area.
            for (column, row) in [
                (area.x + 1, area.y),
                (area.x + 5, area.y - 1),
                (area.x + 9, area.y - 1),
                (area.x + 5, area.y + 1),
                (area.x + 9, area.y + 1),
            ] {
                let elsewhere = Milestone::ALL.into_iter().find(|other| *other != stage);
                fixture.app.select_milestone(elsewhere.unwrap());
                click(&mut fixture.app, column, row)?;
                assert_eq!(fixture.app.detail.milestone, stage);
                assert!(!fixture.app.modal.is_open());
                assert_eq!(fixture.app.screen, Screen::Detail);
            }
        }
    }
    assert_eq!(fs::read(&record)?, before);
    Ok(())
}

#[test]
fn action_hitboxes_cover_their_drawn_labels_and_vanish_while_a_prompt_is_open() -> Result<()> {
    for (width, height) in [(40, 18), (60, 24), (100, 35)] {
        for stage in 0..=4 {
            let mut fixture = Fixture::new(stage)?;
            for milestone in Milestone::ALL {
                fixture.app.select_milestone(milestone);
                let terminal = render(&mut fixture.app, width, height)?;
                let actions = fixture.app.actions();
                let hitboxes = &fixture.app.detail.action_hitboxes;
                assert_eq!(hitboxes.len(), actions.len());
                for (area, action) in hitboxes.iter().zip(&actions) {
                    assert!(
                        drawn(&terminal, *area).contains(action.label()),
                        "{} is not under its hitbox at {width}x{height}",
                        action.label()
                    );
                }
            }
        }
    }
    let mut fixture = Fixture::new(2)?;
    render(&mut fixture.app, 100, 35)?;
    assert!(!fixture.app.detail.action_hitboxes.is_empty());
    press(&mut fixture.app, KeyCode::Enter)?;
    render(&mut fixture.app, 100, 35)?;
    assert!(fixture.app.modal.prompt.is_some());
    assert!(fixture.app.detail.action_hitboxes.is_empty());
    press(&mut fixture.app, KeyCode::Esc)?;
    render(&mut fixture.app, 100, 35)?;
    assert!(!fixture.app.detail.action_hitboxes.is_empty());
    assert_eq!(fixture.app.detail.milestone, Milestone::Dev);
    Ok(())
}

#[test]
fn hovering_an_action_selects_it_and_clicking_runs_it() -> Result<()> {
    for milestone in Milestone::ALL {
        let mut fixture = Fixture::new(4)?;
        fixture.app.select_milestone(milestone);
        // Completed Jira and PR stages offer a second action; the others have one.
        let index = usize::from(matches!(milestone, Milestone::Jira | Milestone::Pr));
        render(&mut fixture.app, 40, 18)?;
        let area = fixture.app.detail.action_hitboxes[index];
        let (column, row) = (area.x + 1, area.bottom() - 1);
        mouse(&mut fixture.app, MouseEventKind::Moved, column, row)?;
        assert_eq!(fixture.app.detail.action, index);
        assert_eq!(fixture.app.detail.milestone, milestone);
        assert!(!fixture.app.modal.is_open());
        click(&mut fixture.app, column, row)?;
        let prompt = &fixture.app.modal.prompt;
        match milestone {
            Milestone::Spec => assert_eq!(fixture.app.screen, Screen::Reader),
            Milestone::Jira => assert!(matches!(prompt, Some(Prompt::Jira { .. }))),
            Milestone::Dev => assert!(matches!(prompt, Some(Prompt::Agent { .. }))),
            Milestone::Pr => assert!(matches!(prompt, Some(Prompt::Pr { .. }))),
        }
    }
    Ok(())
}

#[test]
fn scrolling_moves_between_milestones() -> Result<()> {
    let mut fixture = Fixture::new(4)?;
    fixture.app.select_milestone(Milestone::Spec);
    mouse(&mut fixture.app, MouseEventKind::ScrollDown, 0, 0)?;
    assert_eq!(fixture.app.detail.milestone, Milestone::Jira);
    mouse(&mut fixture.app, MouseEventKind::ScrollUp, 0, 0)?;
    mouse(&mut fixture.app, MouseEventKind::ScrollUp, 0, 0)?;
    assert_eq!(fixture.app.detail.milestone, Milestone::Spec);
    Ok(())
}
