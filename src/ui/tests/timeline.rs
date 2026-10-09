use super::support::*;
use super::*;

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
        assert_eq!(fixture.app.screen, Screen::Detail);
        assert_eq!(fixture.app.detail.milestone, expected);
        fixture.app.select_milestone(Milestone::Spec);
        press(&mut fixture.app, KeyCode::Esc)?;
        fixture.open()?;
        assert_eq!(fixture.app.detail.milestone, expected);
    }
    Ok(())
}

#[test]
fn navigation_reaches_locked_and_completed_milestones_without_mutating_records() -> Result<()> {
    for stage in [0, 4] {
        let mut fixture = Fixture::new(stage)?;
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
            assert_eq!(fixture.app.detail.milestone, expected);
            assert_eq!(fixture.app.detail.action, 0);
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
    let before = fs::read(
        fixture
            .root
            .join("items")
            .join(format!("{}.json", fixture.id)),
    )?;
    for milestone in [Milestone::Jira, Milestone::Dev, Milestone::Pr] {
        fixture.app.select_milestone(milestone);
        press(&mut fixture.app, KeyCode::Enter)?;
        assert!(fixture.app.modal.prompt.is_none());
        assert_eq!(fixture.app.screen, Screen::Detail);
        assert_eq!(fixture.app.detail.milestone, milestone);
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
fn external_progress_follows_the_next_step_but_preserves_history_selection() -> Result<()> {
    let mut fixture = Fixture::new(0)?;
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
        assert_eq!(fixture.app.detail.milestone, next);
        fixture.app.refresh()?;
        assert_eq!(fixture.app.detail.milestone, next);
    }

    let mut fixture = Fixture::new(1)?;
    fixture.app.select_milestone(Milestone::Spec);
    fixture.app.detail.action = 1;
    fixture.app.store.update(
        &fixture.id,
        Change::Jira {
            key: "PAY-123".into(),
            url: None,
        },
    )?;
    fixture.app.refresh()?;
    assert_eq!(fixture.app.detail.milestone, Milestone::Spec);
    assert_eq!(fixture.app.detail.action, 1);
    Ok(())
}

#[test]
fn editing_jira_prefills_and_preserves_its_url() -> Result<()> {
    let mut fixture = Fixture::new(2)?;
    fixture.app.select_milestone(Milestone::Jira);
    fixture.app.detail.action = 1;
    press(&mut fixture.app, KeyCode::Enter)?;
    assert!(matches!(
        fixture.app.modal.prompt,
        Some(Prompt::Jira { .. })
    ));
    assert_eq!(fixture.app.modal.input, "PAY-123");
    fixture.app.modal.input = "PAY-456".into();
    press(&mut fixture.app, KeyCode::Enter)?;
    assert!(matches!(
        fixture.app.modal.prompt,
        Some(Prompt::JiraUrl { .. })
    ));
    assert_eq!(
        fixture.app.modal.input,
        "https://jira.example/browse/PAY-123"
    );
    press(&mut fixture.app, KeyCode::Enter)?;
    let record = fixture.app.store.get(&fixture.id)?;
    assert_eq!(record.jira.key.as_deref(), Some("PAY-456"));
    assert_eq!(
        record.jira.url.as_deref(),
        Some("https://jira.example/browse/PAY-123")
    );
    assert_eq!(fixture.app.detail.milestone, Milestone::Jira);
    Ok(())
}

#[test]
fn editing_implementation_preserves_branch_and_draft_pr_progress() -> Result<()> {
    let mut fixture = Fixture::new(4)?;
    fixture.app.select_milestone(Milestone::Dev);
    press(&mut fixture.app, KeyCode::Enter)?;
    assert!(matches!(
        fixture.app.modal.prompt,
        Some(Prompt::Agent { .. })
    ));
    assert_eq!(fixture.app.modal.input, "implementor");
    fixture.app.modal.input = "replacement agent".into();
    press(&mut fixture.app, KeyCode::Enter)?;
    assert!(matches!(
        fixture.app.modal.prompt,
        Some(Prompt::Branch { .. })
    ));
    assert_eq!(fixture.app.modal.input, "feature/payment-retries");
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
    assert_eq!(fixture.app.detail.milestone, Milestone::Dev);
    Ok(())
}
