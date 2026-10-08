use super::*;

struct Fixture {
    app: App,
    id: String,
    root: std::path::PathBuf,
}

impl Fixture {
    fn new(stage: usize) -> Result<Self> {
        let root = std::env::temp_dir().join(format!("herdr-inbox-timeline-{}", Uuid::new_v4()));
        let store = configured_store(root.clone())?;
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
        Ok(Self {
            app: App::new(store)?,
            id: record.id,
            root,
        })
    }

    fn open(&mut self) -> Result<()> {
        press(&mut self.app, KeyCode::Enter)
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

#[test]
fn opening_selects_the_next_milestone_through_the_whole_workflow() -> Result<()> {
    for (stage, expected) in [
        Milestone::Spec,
        Milestone::Jira,
        Milestone::Dev,
        Milestone::Pr,
        Milestone::Pr,
    ]
    .into_iter()
    .enumerate()
    {
        let mut fixture = Fixture::new(stage)?;
        fixture.open()?;
        assert_eq!(fixture.app.screen, Screen::Detail);
        assert_eq!(fixture.app.milestone_selected, expected);
        fixture.app.select_milestone(Milestone::Spec);
        press(&mut fixture.app, KeyCode::Esc)?;
        fixture.open()?;
        assert_eq!(fixture.app.milestone_selected, expected);
    }
    Ok(())
}

#[test]
fn navigation_reaches_locked_and_completed_milestones_without_mutating_records() -> Result<()> {
    for stage in [0, 4] {
        let mut fixture = Fixture::new(stage)?;
        fixture.open()?;
        let before = fs::read(
            fixture
                .root
                .join("items")
                .join(format!("{}.json", fixture.id)),
        )?;
        fixture.app.select_milestone(Milestone::Spec);
        for (key, expected) in [
            (KeyCode::Char('j'), Milestone::Jira),
            (KeyCode::Down, Milestone::Dev),
            (KeyCode::Char('j'), Milestone::Pr),
            (KeyCode::Down, Milestone::Pr),
            (KeyCode::Char('k'), Milestone::Dev),
            (KeyCode::Up, Milestone::Jira),
            (KeyCode::Char('k'), Milestone::Spec),
            (KeyCode::Up, Milestone::Spec),
        ] {
            press(&mut fixture.app, key)?;
            assert_eq!(fixture.app.milestone_selected, expected);
            assert_eq!(fixture.app.action_selected, 0);
        }
        assert_eq!(
            fs::read(
                fixture
                    .root
                    .join("items")
                    .join(format!("{}.json", fixture.id))
            )?,
            before
        );
    }
    Ok(())
}

#[test]
fn each_milestone_exposes_only_its_own_available_actions() -> Result<()> {
    for (stage, expected) in [
        [
            vec![
                DetailAction::Finish,
                DetailAction::ReadSpec,
                DetailAction::RefineSpec,
            ],
            vec![],
            vec![],
            vec![],
        ],
        [
            vec![DetailAction::ReadSpec, DetailAction::RefineSpec],
            vec![DetailAction::Jira],
            vec![],
            vec![],
        ],
        [
            vec![DetailAction::ReadSpec, DetailAction::RefineSpec],
            vec![DetailAction::OpenJira, DetailAction::UpdateJira],
            vec![DetailAction::Implement],
            vec![],
        ],
        [
            vec![DetailAction::ReadSpec, DetailAction::RefineSpec],
            vec![DetailAction::OpenJira, DetailAction::UpdateJira],
            vec![DetailAction::UpdateImplementation],
            vec![DetailAction::Pr],
        ],
        [
            vec![DetailAction::ReadSpec, DetailAction::RefineSpec],
            vec![DetailAction::OpenJira, DetailAction::UpdateJira],
            vec![DetailAction::UpdateImplementation],
            vec![DetailAction::ReviewPr, DetailAction::UpdatePr],
        ],
    ]
    .into_iter()
    .enumerate()
    {
        let mut fixture = Fixture::new(stage)?;
        fixture.open()?;
        for (milestone, actions) in Milestone::ALL.into_iter().zip(expected) {
            fixture.app.select_milestone(milestone);
            assert_eq!(
                fixture.app.actions(),
                actions,
                "stage {stage}, {milestone:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn enter_on_a_locked_milestone_is_safe() -> Result<()> {
    let mut fixture = Fixture::new(0)?;
    fixture.open()?;
    let before = fs::read(
        fixture
            .root
            .join("items")
            .join(format!("{}.json", fixture.id)),
    )?;
    for milestone in [Milestone::Jira, Milestone::Dev, Milestone::Pr] {
        fixture.app.select_milestone(milestone);
        press(&mut fixture.app, KeyCode::Enter)?;
        assert!(fixture.app.prompt.is_none());
        assert_eq!(fixture.app.screen, Screen::Detail);
        assert_eq!(fixture.app.milestone_selected, milestone);
    }
    assert_eq!(
        fs::read(
            fixture
                .root
                .join("items")
                .join(format!("{}.json", fixture.id))
        )?,
        before
    );
    Ok(())
}

#[test]
fn timeline_nodes_share_one_column_and_clicks_select_without_acting() -> Result<()> {
    let mut fixture = Fixture::new(1)?;
    fixture.open()?;
    let before = fs::read(
        fixture
            .root
            .join("items")
            .join(format!("{}.json", fixture.id)),
    )?;
    let mut terminal = Terminal::new(TestBackend::new(100, 35))?;
    terminal.draw(|frame| draw::draw(frame, &mut fixture.app))?;
    let milestones = fixture.app.milestone_hitboxes.clone();
    assert_eq!(milestones.len(), 4);
    let node_x = milestones[0].1.x + 7;
    for (milestone, area) in &milestones {
        assert_eq!(area.x + 7, node_x);
        assert!(matches!(
            terminal.backend().buffer()[(node_x, area.y)].symbol(),
            "●" | "◉" | "○" | "◐"
        ));
        handle_mouse(
            &mut fixture.app,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: area.x + 1,
                row: area.y,
                modifiers: KeyModifiers::NONE,
            },
            35,
        )?;
        assert_eq!(fixture.app.milestone_selected, *milestone);
        assert!(fixture.app.prompt.is_none());
    }
    let buffer = terminal.backend().buffer();
    let selected_y = milestones
        .iter()
        .find(|(stage, _)| *stage == Milestone::Jira)
        .unwrap()
        .1
        .y;
    for pair in milestones.windows(2) {
        for y in pair[0].1.y + 1..pair[1].1.y {
            if y.abs_diff(selected_y) <= 1 {
                continue;
            }
            assert_eq!(buffer[(node_x, y)].symbol(), "│");
        }
    }
    assert!(!buffer.content().iter().any(|cell| cell.symbol() == "└"));
    assert_eq!(
        fs::read(
            fixture
                .root
                .join("items")
                .join(format!("{}.json", fixture.id))
        )?,
        before
    );
    Ok(())
}

#[test]
fn external_progress_follows_the_next_step_but_preserves_history_selection() -> Result<()> {
    let mut fixture = Fixture::new(0)?;
    fixture.open()?;
    for (change, next) in [
        (Change::Finish { title: None }, Milestone::Jira),
        (
            Change::Jira {
                key: "PAY-123".into(),
                url: None,
            },
            Milestone::Dev,
        ),
        (
            Change::Implement {
                agent: None,
                branch: None,
            },
            Milestone::Pr,
        ),
    ] {
        fixture.app.store.update(&fixture.id, change)?;
        fixture.app.refresh()?;
        assert_eq!(fixture.app.milestone_selected, next);
        fixture.app.refresh()?;
        assert_eq!(fixture.app.milestone_selected, next);
    }

    let mut fixture = Fixture::new(1)?;
    fixture.open()?;
    fixture.app.select_milestone(Milestone::Spec);
    fixture.app.action_selected = 1;
    fixture.app.store.update(
        &fixture.id,
        Change::Jira {
            key: "PAY-123".into(),
            url: None,
        },
    )?;
    fixture.app.refresh()?;
    assert_eq!(fixture.app.milestone_selected, Milestone::Spec);
    assert_eq!(fixture.app.action_selected, 1);
    Ok(())
}

#[test]
fn editing_jira_prefills_and_preserves_its_url() -> Result<()> {
    let mut fixture = Fixture::new(2)?;
    fixture.open()?;
    fixture.app.select_milestone(Milestone::Jira);
    fixture.app.action_selected = 1;
    press(&mut fixture.app, KeyCode::Enter)?;
    assert!(matches!(fixture.app.prompt, Some(Prompt::Jira { .. })));
    assert_eq!(fixture.app.input, "PAY-123");
    fixture.app.input = "PAY-456".into();
    press(&mut fixture.app, KeyCode::Enter)?;
    assert!(matches!(fixture.app.prompt, Some(Prompt::JiraUrl { .. })));
    assert_eq!(fixture.app.input, "https://jira.example/browse/PAY-123");
    press(&mut fixture.app, KeyCode::Enter)?;
    let record = fixture.app.store.get(&fixture.id)?;
    assert_eq!(record.jira.key.as_deref(), Some("PAY-456"));
    assert_eq!(
        record.jira.url.as_deref(),
        Some("https://jira.example/browse/PAY-123")
    );
    assert_eq!(fixture.app.milestone_selected, Milestone::Jira);
    Ok(())
}

#[test]
fn editing_implementation_preserves_branch_and_draft_pr_progress() -> Result<()> {
    let mut fixture = Fixture::new(4)?;
    fixture.open()?;
    fixture.app.select_milestone(Milestone::Dev);
    press(&mut fixture.app, KeyCode::Enter)?;
    assert!(matches!(fixture.app.prompt, Some(Prompt::Agent { .. })));
    assert_eq!(fixture.app.input, "implementor");
    fixture.app.input = "replacement agent".into();
    press(&mut fixture.app, KeyCode::Enter)?;
    assert!(matches!(fixture.app.prompt, Some(Prompt::Branch { .. })));
    assert_eq!(fixture.app.input, "feature/payment-retries");
    press(&mut fixture.app, KeyCode::Enter)?;
    let record = fixture.app.store.get(&fixture.id)?;
    assert_eq!(
        record.implementation.agent.as_deref(),
        Some("replacement agent")
    );
    assert_eq!(
        record.implementation.branch.as_deref(),
        Some("feature/payment-retries")
    );
    assert_eq!(record.implementation.status, "draft_pr");
    assert_eq!(record.pr.status, "draft");
    assert_eq!(
        record.pr.url.as_deref(),
        Some("https://github.example/org/repo/pull/42")
    );
    assert_eq!(fixture.app.milestone_selected, Milestone::Dev);
    Ok(())
}

#[test]
fn small_terminal_keeps_every_milestone_and_spec_action_visible() -> Result<()> {
    let mut fixture = Fixture::new(0)?;
    fixture.open()?;
    let mut terminal = Terminal::new(TestBackend::new(40, 18))?;
    terminal.draw(|frame| draw::draw(frame, &mut fixture.app))?;
    let lines: Vec<String> = terminal
        .backend()
        .buffer()
        .content()
        .chunks(40)
        .map(|cells| cells.iter().map(|cell| cell.symbol()).collect())
        .collect();
    assert_eq!(fixture.app.milestone_hitboxes.len(), 4);
    for (label, status) in [
        ("SPEC", "active"),
        ("JIRA", "wait"),
        ("DEV", "locked"),
        ("PR", "locked"),
    ] {
        assert!(
            lines
                .iter()
                .any(|line| line.contains(label) && line.contains(status)),
            "missing {label}"
        );
    }
    assert_eq!(fixture.app.action_hitboxes.len(), 3);
    for (area, action) in fixture.app.action_hitboxes.iter().zip([
        "Seal the spec",
        "Read the scroll",
        "Refine the spec",
    ]) {
        let buffer = terminal.backend().buffer();
        let text = (area.y..area.bottom())
            .map(|y| {
                (area.x..area.right())
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains(action), "missing {action}");
    }
    assert!(
        fixture
            .app
            .action_hitboxes
            .iter()
            .all(|area| area.bottom() <= 17)
    );
    Ok(())
}
