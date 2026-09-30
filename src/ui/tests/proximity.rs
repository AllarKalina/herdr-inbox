use super::*;

struct Fixture {
    app: App,
    root: std::path::PathBuf,
}

impl Fixture {
    fn new(stage: usize) -> Result<Self> {
        let root = std::env::temp_dir().join(format!("herdr-inbox-proximity-{}", Uuid::new_v4()));
        let store = Store::new(root.clone());
        let record = store.start("Payment retries", None, None)?;
        if stage >= 1 {
            store.update(&record.id, Change::Finish { title: None })?;
        }
        if stage >= 2 {
            store.update(
                &record.id,
                Change::Jira {
                    key: "PAY-123".into(),
                    url: Some("https://jira.example/browse/PAY-123".into()),
                },
            )?;
        }
        if stage >= 3 {
            store.update(
                &record.id,
                Change::Implement {
                    agent: Some("implementor".into()),
                    branch: Some("feature/payment-retries".into()),
                },
            )?;
        }
        if stage >= 4 {
            store.update(
                &record.id,
                Change::Pr {
                    url: "https://github.example/org/repo/pull/42".into(),
                },
            )?;
        }
        let mut app = App::new(store)?;
        press(&mut app, KeyCode::Enter)?;
        Ok(Self { app, root })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn press(app: &mut App, code: KeyCode) -> Result<()> {
    handle_key(app, KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn render(app: &mut App, width: u16, height: u16) -> Result<Terminal<TestBackend>> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| draw::draw(frame, app))?;
    Ok(terminal)
}

fn row(terminal: &Terminal<TestBackend>, y: u16) -> String {
    let buffer = terminal.backend().buffer();
    (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol())
        .collect()
}

fn node(app: &App, stage: Milestone) -> ratatui::layout::Rect {
    app.milestone_hitboxes
        .iter()
        .find(|(milestone, _)| *milestone == stage)
        .expect("every stage remains visible")
        .1
}

fn assert_continuous_spine(app: &App, terminal: &Terminal<TestBackend>) {
    let buffer = terminal.backend().buffer();
    let milestones = &app.milestone_hitboxes;
    assert_eq!(milestones.len(), 4);
    for pair in milestones.windows(2) {
        let x = pair[0].1.x + 7;
        assert_eq!(x, pair[1].1.x + 7);
        assert!(pair[0].1.y < pair[1].1.y);
        for y in pair[0].1.y + 1..pair[1].1.y {
            assert_eq!(buffer[(x, y)].symbol(), "│", "broken spine at {x}, {y}");
        }
    }
}

#[test]
fn actions_expand_beside_only_their_selected_milestone_at_every_size() -> Result<()> {
    for (width, height) in [(40, 18), (60, 24), (100, 35)] {
        for stage in [0, 1, 2, 3, 4] {
            let mut fixture = Fixture::new(stage)?;
            for (index, milestone) in Milestone::ALL.into_iter().enumerate() {
                fixture.app.select_milestone(milestone);
                let terminal = render(&mut fixture.app, width, height)?;
                assert_continuous_spine(&fixture.app, &terminal);
                let owner = node(&fixture.app, milestone);
                let actions = fixture.app.actions();
                assert_eq!(fixture.app.action_hitboxes.len(), actions.len());
                for (offset, (area, action)) in
                    fixture.app.action_hitboxes.iter().zip(&actions).enumerate()
                {
                    assert_eq!(area.x, owner.x + 9);
                    assert_eq!(area.y, owner.y + 2 + offset as u16);
                    assert!(row(&terminal, area.y).contains(action.label()));
                    if let Some(next) = Milestone::ALL.get(index + 1) {
                        assert!(area.bottom() <= node(&fixture.app, *next).y);
                    }
                }
                let selected_label = &terminal.backend().buffer()[(owner.x, owner.y)];
                let status = &terminal.backend().buffer()[(owner.x + 7, owner.y)];
                assert_eq!(selected_label.bg, Color::Cyan);
                assert_eq!(status.bg, Color::Reset);
                assert_eq!(node(&fixture.app, Milestone::Pr).x, owner.x);
            }
        }
    }
    Ok(())
}

#[test]
fn inline_mouse_buttons_execute_the_selected_milestones_action() -> Result<()> {
    for milestone in Milestone::ALL {
        let mut fixture = Fixture::new(4)?;
        fixture.app.select_milestone(milestone);
        let index = usize::from(matches!(milestone, Milestone::Jira | Milestone::Pr));
        render(&mut fixture.app, 40, 18)?;
        let area = fixture.app.action_hitboxes[index];
        handle_mouse(
            &mut fixture.app,
            MouseEvent {
                kind: MouseEventKind::Moved,
                column: area.x + 1,
                row: area.y,
                modifiers: KeyModifiers::NONE,
            },
            18,
        )?;
        assert_eq!(fixture.app.action_selected, index);
        assert_eq!(fixture.app.milestone_selected, milestone);
        assert!(fixture.app.prompt.is_none());
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
        match milestone {
            Milestone::Spec => assert_eq!(fixture.app.screen, Screen::Reader),
            Milestone::Jira => assert!(matches!(fixture.app.prompt, Some(Prompt::Jira { .. }))),
            Milestone::Dev => assert!(matches!(fixture.app.prompt, Some(Prompt::Agent { .. }))),
            Milestone::Pr => assert!(matches!(fixture.app.prompt, Some(Prompt::Pr { .. }))),
        }
    }
    Ok(())
}

#[test]
fn locked_milestones_show_local_prerequisites_instead_of_actions() -> Result<()> {
    for (milestone, hint) in [
        (Milestone::Jira, "Finish the spec"),
        (Milestone::Dev, "Link Jira"),
        (Milestone::Pr, "Start implementation"),
    ] {
        let mut fixture = Fixture::new(0)?;
        fixture.app.select_milestone(milestone);
        let terminal = render(&mut fixture.app, 40, 18)?;
        assert_continuous_spine(&fixture.app, &terminal);
        assert!(fixture.app.action_hitboxes.is_empty());
        let owner = node(&fixture.app, milestone);
        assert!(row(&terminal, owner.y + 1).contains(hint));
        press(&mut fixture.app, KeyCode::Enter)?;
        assert!(fixture.app.prompt.is_none());
        assert_eq!(fixture.app.current().unwrap().spec, "in_progress");
    }
    Ok(())
}

#[test]
fn prompts_replace_local_actions_and_keep_other_milestones_visible() -> Result<()> {
    for (width, height) in [(40, 18), (60, 24), (100, 35)] {
        for milestone in [Milestone::Jira, Milestone::Dev, Milestone::Pr] {
            let mut fixture = Fixture::new(4)?;
            fixture.app.select_milestone(milestone);
            fixture.app.action_selected = usize::from(milestone != Milestone::Dev);
            press(&mut fixture.app, KeyCode::Enter)?;
            let label = fixture.app.prompt.as_ref().unwrap().label();
            let terminal = render(&mut fixture.app, width, height)?;
            assert_continuous_spine(&fixture.app, &terminal);
            assert!(fixture.app.action_hitboxes.is_empty());
            let owner = node(&fixture.app, milestone);
            assert!(row(&terminal, owner.y + 2).contains(label));
            assert!(row(&terminal, owner.y + 4).contains("Enter save · Esc cancel"));
            press(&mut fixture.app, KeyCode::Esc)?;
            render(&mut fixture.app, width, height)?;
            assert!(!fixture.app.action_hitboxes.is_empty());
            assert_eq!(fixture.app.milestone_selected, milestone);
        }
    }
    Ok(())
}

#[test]
fn long_inline_input_keeps_the_typed_suffix_and_cursor_visible() -> Result<()> {
    let mut fixture = Fixture::new(2)?;
    fixture.app.select_milestone(Milestone::Jira);
    fixture.app.action_selected = 1;
    press(&mut fixture.app, KeyCode::Enter)?;
    fixture.app.input = format!("{}-VISIBLE-END", "discardable-prefix".repeat(10));
    let terminal = render(&mut fixture.app, 40, 18)?;
    let owner = node(&fixture.app, Milestone::Jira);
    let input_row = row(&terminal, owner.y + 3);
    assert!(input_row.contains("-VISIBLE-END█"));
    assert!(!input_row.contains("discardable-prefix"));
    assert_continuous_spine(&fixture.app, &terminal);
    Ok(())
}

#[test]
fn delete_confirmation_stays_beside_the_selected_stage_until_cancelled() -> Result<()> {
    for (width, height) in [(40, 18), (100, 35)] {
        for milestone in Milestone::ALL {
            let mut fixture = Fixture::new(4)?;
            fixture.app.select_milestone(milestone);
            press(&mut fixture.app, KeyCode::Char('d'))?;
            let terminal = render(&mut fixture.app, width, height)?;
            assert_continuous_spine(&fixture.app, &terminal);
            let owner = node(&fixture.app, milestone);
            assert!(row(&terminal, owner.y + 2).contains("Delete Payment retries?"));
            assert!(row(&terminal, owner.y + 4).contains("Enter delete · Esc cancel"));
            assert!(fixture.app.action_hitboxes.is_empty());
            assert_eq!(fixture.app.store.list()?.len(), 1);
            press(&mut fixture.app, KeyCode::Esc)?;
            render(&mut fixture.app, width, height)?;
            assert_eq!(fixture.app.milestone_selected, milestone);
            assert_eq!(fixture.app.store.list()?.len(), 1);
            assert!(!fixture.app.action_hitboxes.is_empty());
        }
    }
    Ok(())
}

#[test]
fn linked_spec_confirmation_fits_without_hiding_later_nodes() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-linked-confirm-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    let record = store.start("Linked spec", None, Some(root.join("external.md")))?;
    let mut fixture = Fixture {
        app: App::new(store)?,
        root,
    };
    press(&mut fixture.app, KeyCode::Enter)?;
    press(&mut fixture.app, KeyCode::Char('d'))?;
    let terminal = render(&mut fixture.app, 40, 18)?;
    assert_continuous_spine(&fixture.app, &terminal);
    let owner = node(&fixture.app, Milestone::Spec);
    assert!(row(&terminal, owner.y + 4).contains("Linked spec stays in place."));
    assert!(row(&terminal, owner.y + 5).contains("Enter delete · Esc cancel"));
    press(&mut fixture.app, KeyCode::Esc)?;
    assert!(record.spec_path.is_file());
    Ok(())
}
