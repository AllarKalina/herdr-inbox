use super::proximity::{Fixture, coordinates, node, render, row};
use super::*;
use ratatui::style::Modifier;

#[test]
fn selected_circle_grows_inside_reserved_space_without_filling_the_header() -> Result<()> {
    for (width, height) in [(40, 18), (60, 24), (100, 35)] {
        let mut fixture = Fixture::new(4)?;
        render(&mut fixture.app, width, height)?;
        let fixed = coordinates(&fixture.app);
        for selected in Milestone::ALL {
            fixture.app.select_milestone(selected);
            let terminal = render(&mut fixture.app, width, height)?;
            assert_eq!(coordinates(&fixture.app), fixed);
            let buffer = terminal.backend().buffer();
            for stage in Milestone::ALL {
                let area = node(&fixture.app, stage);
                let center_x = area.x + 7;
                let ring_cells: Vec<_> = (area.y - 1..=area.y + 1)
                    .flat_map(|y| (center_x - 2..=center_x + 2).map(move |x| (x, y)))
                    .filter(|&(x, y)| (x, y) != (center_x, area.y))
                    .map(|position| &buffer[position])
                    .filter(|cell| cell.fg == Color::Cyan && !cell.symbol().trim().is_empty())
                    .collect();
                if stage == selected {
                    assert!(
                        ring_cells.len() >= 4,
                        "selected circle should occupy multiple cells"
                    );
                    for y in [area.y - 1, area.y + 1] {
                        assert!(
                            (center_x - 2..=center_x + 2).any(|x| {
                                let cell = &buffer[(x, y)];
                                cell.fg == Color::Cyan && !cell.symbol().trim().is_empty()
                            }),
                            "selected circle must retain both its top and bottom arcs"
                        );
                    }
                } else {
                    assert!(ring_cells.is_empty(), "only selected circle should grow");
                }
                for x in area.x..area.x + 4 {
                    assert_eq!(buffer[(x, area.y)].bg, Color::Reset);
                }
                assert!(matches!(
                    buffer[(center_x, area.y)].symbol(),
                    "●" | "◉" | "○" | "◐"
                ));
            }
            if let Some(action) = fixture.app.action_hitboxes.first() {
                let cell = &buffer[(action.x, action.y)];
                assert_eq!(cell.symbol(), "✦");
                assert_eq!(cell.fg, Color::Cyan);
                assert_eq!(cell.bg, Color::Reset);
                assert!(
                    cell.modifier
                        .contains(Modifier::BOLD | Modifier::UNDERLINED)
                );
                assert!(!buffer.content().iter().any(|cell| cell.symbol() == "▸"));
            }
        }
    }
    Ok(())
}

#[test]
fn every_edge_of_the_larger_circle_selects_without_activating_an_action() -> Result<()> {
    let mut fixture = Fixture::new(4)?;
    render(&mut fixture.app, 40, 18)?;
    for stage in Milestone::ALL {
        let area = node(&fixture.app, stage);
        for (column, row) in [
            (area.x + 5, area.y - 1),
            (area.x + 9, area.y - 1),
            (area.x + 5, area.y + 1),
            (area.x + 9, area.y + 1),
        ] {
            fixture.app.select_milestone(if stage == Milestone::Spec {
                Milestone::Pr
            } else {
                Milestone::Spec
            });
            handle_mouse(
                &mut fixture.app,
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column,
                    row,
                    modifiers: KeyModifiers::NONE,
                },
            )?;
            assert_eq!(fixture.app.milestone_selected, stage);
            assert!(fixture.app.prompt.is_none());
            assert_eq!(fixture.app.screen, Screen::Detail);
        }
    }
    Ok(())
}

#[test]
fn launched_specs_do_not_repeat_their_workspace_or_client_in_progress() -> Result<()> {
    let mut fixture = Fixture::new(1)?;
    let id = fixture.app.current().unwrap().id.clone();
    fixture.app.store.update(
        &id,
        Change::Launch(
            Box::new(crate::store::Launch {
                status: crate::store::LaunchStatus::PromptSent,
                harness: "codex".into(),
                workspace: "ai-boiler-room".into(),
                workspace_id: None,
                tab_id: None,
                pane_id: None,
                agent: None,
                model: "gpt-6-sol".into(),
                effort: "high".into(),
                prompt: "Test spec prompt".into(),
                error: None,
            }),
            fixture.app.current().unwrap().spec_path.clone(),
        ),
    )?;
    fixture.app.refresh()?;
    assert_eq!(
        Milestone::Spec.context(fixture.app.current().unwrap(), true),
        ""
    );
    for (width, height) in [(40, 18), (60, 24), (100, 35)] {
        for stage in Milestone::ALL {
            fixture.app.select_milestone(stage);
            let terminal = render(&mut fixture.app, width, height)?;
            let text = (0..height)
                .map(|y| row(&terminal, y))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(!text.contains("ai-boiler-room"));
            assert!(!text.contains("codex"));
            assert!(!text.contains("gpt-6-sol"));
        }
    }
    Ok(())
}
