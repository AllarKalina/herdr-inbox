use super::*;

pub(super) struct Fixture {
    pub(super) app: App,
    root: std::path::PathBuf,
}

impl Fixture {
    pub(super) fn new(stage: usize) -> Result<Self> {
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

pub(super) fn press(app: &mut App, code: KeyCode) -> Result<()> {
    handle_key(app, KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

pub(super) fn render(app: &mut App, width: u16, height: u16) -> Result<Terminal<TestBackend>> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| draw::draw(frame, app))?;
    Ok(terminal)
}

pub(super) fn row(terminal: &Terminal<TestBackend>, y: u16) -> String {
    let buffer = terminal.backend().buffer();
    (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol())
        .collect()
}

pub(super) fn node(app: &App, stage: Milestone) -> ratatui::layout::Rect {
    let area = app
        .milestone_hitboxes
        .iter()
        .find(|(milestone, _)| *milestone == stage)
        .expect("every stage remains visible")
        .1;
    ratatui::layout::Rect::new(area.x, area.y + area.height / 2, area.width, 1)
}

fn assert_continuous_spine(app: &App, terminal: &Terminal<TestBackend>) {
    let buffer = terminal.backend().buffer();
    let milestones = &app.milestone_hitboxes;
    assert_eq!(milestones.len(), 4);
    for pair in milestones.windows(2) {
        let x = pair[0].1.x + 7;
        assert_eq!(x, pair[1].1.x + 7);
        let first = node(app, pair[0].0);
        let next = node(app, pair[1].0);
        let selected = node(app, app.milestone_selected);
        assert!(first.y < next.y);
        for y in first.y + 1..next.y {
            if y.abs_diff(selected.y) <= 1 {
                continue;
            }
            assert_eq!(buffer[(x, y)].symbol(), "│", "broken spine at {x}, {y}");
        }
    }
}

pub(super) fn coordinates(app: &App) -> Vec<(Milestone, ratatui::layout::Rect)> {
    app.milestone_hitboxes.clone()
}

fn actions_origin(app: &App) -> (u16, u16) {
    let first = node(app, Milestone::Spec);
    let owner = node(app, app.milestone_selected);
    if roomy(app) {
        (owner.x + 11, owner.y + 2)
    } else {
        (first.x + 20, first.y + 5)
    }
}

fn roomy(app: &App) -> bool {
    node(app, Milestone::Spec).width > 18
}

fn panel_text(terminal: &Terminal<TestBackend>, x: u16, y: u16, height: u16) -> String {
    let buffer = terminal.backend().buffer();
    (y..y + height)
        .map(|line| {
            (x..buffer.area.width - 2)
                .map(|column| buffer[(column, line)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn selecting_milestones_keeps_nodes_fixed_and_actions_nearby() -> Result<()> {
    for (width, height) in [(40, 18), (60, 24), (100, 35)] {
        for stage in [0, 1, 2, 3, 4] {
            let mut fixture = Fixture::new(stage)?;
            render(&mut fixture.app, width, height)?;
            let fixed = coordinates(&fixture.app);
            for (index, milestone) in Milestone::ALL.into_iter().enumerate() {
                fixture.app.select_milestone(milestone);
                let terminal = render(&mut fixture.app, width, height)?;
                assert_continuous_spine(&fixture.app, &terminal);
                assert_eq!(coordinates(&fixture.app), fixed);
                let owner = node(&fixture.app, milestone);
                let actions = fixture.app.actions();
                assert_eq!(fixture.app.action_hitboxes.len(), actions.len());
                let (x, mut y) = actions_origin(&fixture.app);
                for (area, action) in fixture.app.action_hitboxes.iter().zip(&actions) {
                    assert_eq!(area.x, x);
                    assert_eq!(area.y, y);
                    assert!(
                        panel_text(&terminal, area.x, area.y, area.height).contains(action.label())
                    );
                    y += if roomy(&fixture.app) { 1 } else { 2 };
                    if roomy(&fixture.app)
                        && let Some(next) = Milestone::ALL.get(index + 1)
                    {
                        assert!(area.bottom() <= node(&fixture.app, *next).y);
                    }
                }
                let selected_label = &terminal.backend().buffer()[(owner.x, owner.y)];
                let status = &terminal.backend().buffer()[(owner.x + 7, owner.y)];
                assert_eq!(selected_label.bg, Color::Reset);
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
                row: area.bottom() - 1,
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
                row: area.bottom() - 1,
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
        let first = node(&fixture.app, Milestone::Spec);
        assert!(panel_text(&terminal, first.x + 20, first.y + 1, 4).contains(hint));
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
            render(&mut fixture.app, width, height)?;
            let fixed = coordinates(&fixture.app);
            fixture.app.action_selected = usize::from(milestone != Milestone::Dev);
            press(&mut fixture.app, KeyCode::Enter)?;
            let label = fixture.app.prompt.as_ref().unwrap().label();
            let terminal = render(&mut fixture.app, width, height)?;
            assert_continuous_spine(&fixture.app, &terminal);
            assert_eq!(coordinates(&fixture.app), fixed);
            assert!(fixture.app.action_hitboxes.is_empty());
            let (x, y) = actions_origin(&fixture.app);
            let prompt = panel_text(&terminal, x, y, if roomy(&fixture.app) { 4 } else { 8 });
            assert!(prompt.contains(label.split(" (optional)").next().unwrap()));
            assert!(prompt.contains("Enter save · Esc cancel"));
            press(&mut fixture.app, KeyCode::Esc)?;
            render(&mut fixture.app, width, height)?;
            assert!(!fixture.app.action_hitboxes.is_empty());
            assert_eq!(fixture.app.milestone_selected, milestone);
            assert_eq!(coordinates(&fixture.app), fixed);
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
    let (_, y) = actions_origin(&fixture.app);
    let input_row = row(&terminal, y + 1);
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
            render(&mut fixture.app, width, height)?;
            let fixed = coordinates(&fixture.app);
            press(&mut fixture.app, KeyCode::Char('a'))?;
            let terminal = render(&mut fixture.app, width, height)?;
            assert_continuous_spine(&fixture.app, &terminal);
            assert_eq!(coordinates(&fixture.app), fixed);
            let (x, y) = actions_origin(&fixture.app);
            let prompt = panel_text(&terminal, x, y, if roomy(&fixture.app) { 4 } else { 8 });
            assert!(prompt.contains("Archive Pay"));
            assert!(prompt.contains('?'));
            assert!(prompt.contains("Enter archive · Esc"));
            assert!(fixture.app.action_hitboxes.is_empty());
            assert_eq!(fixture.app.store.list()?.len(), 1);
            press(&mut fixture.app, KeyCode::Esc)?;
            render(&mut fixture.app, width, height)?;
            assert_eq!(fixture.app.milestone_selected, milestone);
            assert_eq!(fixture.app.store.list()?.len(), 1);
            assert!(!fixture.app.action_hitboxes.is_empty());
            assert_eq!(coordinates(&fixture.app), fixed);
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
    for (width, height) in [(40, 18), (60, 24), (100, 35)] {
        for milestone in Milestone::ALL {
            fixture.app.select_milestone(milestone);
            render(&mut fixture.app, width, height)?;
            let fixed = coordinates(&fixture.app);
            press(&mut fixture.app, KeyCode::Char('a'))?;
            let terminal = render(&mut fixture.app, width, height)?;
            assert_continuous_spine(&fixture.app, &terminal);
            assert_eq!(coordinates(&fixture.app), fixed);
            let (x, y) = actions_origin(&fixture.app);
            let prompt = panel_text(&terminal, x, y, if roomy(&fixture.app) { 4 } else { 8 });
            assert!(
                prompt.contains("Linked spec stays intact.") || prompt.contains("Spec stays put.")
            );
            assert!(prompt.contains("Enter archive · Esc"));
            press(&mut fixture.app, KeyCode::Esc)?;
            render(&mut fixture.app, width, height)?;
            assert_eq!(coordinates(&fixture.app), fixed);
        }
    }
    assert!(record.spec_path.is_file());
    Ok(())
}

#[test]
fn workflow_changes_do_not_reposition_milestones() -> Result<()> {
    for (width, height) in [(40, 18), (60, 24), (100, 35)] {
        let mut fixture = Fixture::new(0)?;
        render(&mut fixture.app, width, height)?;
        let fixed = coordinates(&fixture.app);
        let id = fixture.app.current().unwrap().id.clone();
        for change in [
            Change::Finish { title: None },
            Change::Jira {
                key: "PAY-123".into(),
                url: None,
            },
            Change::Implement {
                agent: None,
                branch: None,
            },
            Change::Pr {
                url: "https://github.example/org/repo/pull/42".into(),
            },
        ] {
            fixture.app.store.update(&id, change)?;
            fixture.app.refresh()?;
            render(&mut fixture.app, width, height)?;
            assert_eq!(coordinates(&fixture.app), fixed);
        }
    }
    Ok(())
}
