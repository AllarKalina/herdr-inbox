use super::support::*;
use super::*;

#[test]
fn archiving_from_the_list_needs_a_confirming_enter() -> Result<()> {
    let mut fixture = Fixture::new(0)?;
    let app = &mut fixture.app;
    press(app, KeyCode::Esc)?;
    press(app, KeyCode::Char('a'))?;
    assert!(matches!(app.modal.prompt, Some(Prompt::Archive { .. })));
    press(app, KeyCode::Esc)?;
    assert!(!app.modal.is_open());
    assert_eq!(app.notice.text(), "Cancelled");
    assert!(app.store.get(&fixture.id).is_ok());

    press(app, KeyCode::Char('a'))?;
    // A confirmation is not a text field: stray keys neither type nor confirm.
    press(app, KeyCode::Char('x'))?;
    assert!(app.modal.input.is_empty());
    assert!(app.store.get(&fixture.id).is_ok());
    press(app, KeyCode::Enter)?;
    assert!(app.records.is_empty());
    assert_eq!(app.notice.text(), "Item archived");
    assert_eq!(app.store.archived()?[0].id, fixture.id);
    Ok(())
}

#[test]
fn enter_opens_the_spec_and_r_toggles_the_full_reader() -> Result<()> {
    let mut fixture = Fixture::new(0)?;
    let app = &mut fixture.app;
    assert_eq!(app.screen, Screen::Detail);
    press(app, KeyCode::Char('r'))?;
    assert_eq!(app.screen, Screen::Reader);
    press(app, KeyCode::Char('r'))?;
    assert_eq!(app.screen, Screen::Detail);
    press(app, KeyCode::Char('r'))?;
    press(app, KeyCode::Esc)?;
    assert_eq!(
        app.screen,
        Screen::Detail,
        "Esc leaves the reader, not the item"
    );
    press(app, KeyCode::Esc)?;
    assert_eq!(app.screen, Screen::List);
    assert_eq!(
        app.current().unwrap().id,
        fixture.id,
        "the list returns to the same item"
    );
    Ok(())
}

#[test]
fn typing_through_the_prompts_advances_the_workflow_stage_by_stage() -> Result<()> {
    let mut fixture = Fixture::new(0)?;
    let app = &mut fixture.app;
    assert_eq!(
        app.actions(),
        [
            DetailAction::Finish,
            DetailAction::ReadSpec,
            DetailAction::RefineSpec
        ]
    );
    press(app, KeyCode::Enter)?;
    assert_eq!(app.current().unwrap().spec, "done");
    assert_eq!(
        app.actions(),
        [DetailAction::CreateJira, DetailAction::Jira]
    );

    run_action(app, DetailAction::Jira)?;
    assert!(matches!(app.modal.prompt, Some(Prompt::Jira { .. })));
    // An empty required answer cancels instead of saving nothing.
    press(app, KeyCode::Enter)?;
    assert_eq!(app.notice.text(), "Cancelled");
    assert_eq!(
        app.actions(),
        [DetailAction::CreateJira, DetailAction::Jira]
    );
    run_action(app, DetailAction::Jira)?;
    type_text(app, "ABC-1234")?;
    press(app, KeyCode::Backspace)?;
    press(app, KeyCode::Enter)?;
    assert!(matches!(app.modal.prompt, Some(Prompt::JiraUrl { .. })));
    type_text(app, "https://jira.example/ABC-123")?;
    press(app, KeyCode::Enter)?;
    let jira = &app.current().unwrap().jira;
    assert_eq!(jira.key.as_deref(), Some("ABC-123"));
    assert_eq!(jira.url.as_deref(), Some("https://jira.example/ABC-123"));
    assert_eq!(app.actions(), [DetailAction::Implement]);

    // Agent and branch are optional: two Enters record that work has started.
    press(app, KeyCode::Enter)?;
    assert!(matches!(app.modal.prompt, Some(Prompt::Agent { .. })));
    press(app, KeyCode::Enter)?;
    assert!(matches!(app.modal.prompt, Some(Prompt::Branch { .. })));
    press(app, KeyCode::Enter)?;
    assert_eq!(app.current().unwrap().implementation.status, "in_progress");
    assert_eq!(app.actions(), [DetailAction::Pr]);

    press(app, KeyCode::Enter)?;
    type_text(app, "https://github.example/pr/1")?;
    press(app, KeyCode::Enter)?;
    assert_eq!(
        app.actions(),
        [DetailAction::ReviewPr, DetailAction::UpdatePr]
    );
    Ok(())
}

#[test]
fn settling_an_abandoned_session_needs_confirmation_and_keeps_the_spec_open() -> Result<()> {
    let mut fixture = Fixture::new(0)?;
    let app = &mut fixture.app;
    press(app, KeyCode::Char('x'))?;
    assert!(!app.modal.is_open(), "nothing to settle without a session");
    let launch = crate::store::Launch {
        status: crate::store::LaunchStatus::PromptSent,
        harness: "codex".into(),
        workspace: "ai-boiler-room".into(),
        workspace_id: None,
        tab_id: Some("w1:t1".into()),
        pane_id: None,
        agent: None,
        model: "gpt-6.1-sol".into(),
        effort: "high".into(),
        prompt: String::new(),
        error: None,
    };
    let spec_path = app.current().unwrap().spec_path.clone();
    app.store
        .update(&fixture.id, Change::Launch(Box::new(launch), spec_path))?;
    app.refresh()?;
    press(app, KeyCode::Char('x'))?;
    assert!(matches!(app.modal.prompt, Some(Prompt::Settle { .. })));
    press(app, KeyCode::Esc)?;
    assert!(app.current().unwrap().active_spec_session());
    press(app, KeyCode::Char('x'))?;
    press(app, KeyCode::Enter)?;
    let record = app.current().unwrap();
    assert!(!record.active_spec_session());
    assert_eq!(
        record.spec, "in_progress",
        "settling does not finish the spec"
    );
    Ok(())
}

#[test]
fn scan_results_scroll_and_lead_back_to_the_inbox_or_settings() -> Result<()> {
    let mut fixture = Fixture::new(1)?;
    let app = &mut fixture.app;
    let report = crate::store::ScanReport {
        issues: vec!["Cannot read spec".into(); 40],
        ..Default::default()
    };
    scan::show(app, report);
    assert_eq!(app.screen, Screen::Scan);
    press(app, KeyCode::Char('s'))?;
    assert_eq!(app.screen, Screen::Settings);
    scan::show(app, crate::store::ScanReport::default());
    assert_eq!(
        app.screen,
        Screen::Settings,
        "a clean scan stays where it was"
    );
    app.screen = Screen::Scan;
    press(app, KeyCode::Enter)?;
    assert_eq!(app.screen, Screen::List);
    Ok(())
}

#[test]
fn agent_actions_lead_and_the_ticket_prompt_remembers_the_last_parent() -> Result<()> {
    let mut fixture = Fixture::new(1)?;
    let app = &mut fixture.app;
    press(app, KeyCode::Enter)?;
    assert!(matches!(app.modal.prompt, Some(Prompt::JiraParent { .. })));
    assert!(app.modal.input.is_empty());
    press(app, KeyCode::Esc)?;
    app.store.update_settings(|settings| {
        settings.jira_parent = Some("BT-2000".into());
        Ok(())
    })?;
    app.refresh()?;
    press(app, KeyCode::Enter)?;
    assert_eq!(app.modal.input, "BT-2000");
    // Outside Herdr nothing can be started: the request is refused and the item unchanged.
    press(app, KeyCode::Enter)?;
    assert!(app.notice.text().contains("inside Herdr to start sessions"));
    assert!(!app.should_exit);
    assert_eq!(app.current().unwrap().jira.status, "ready");

    // Development is only launched when a skill is configured; otherwise it is recorded.
    let mut fixture = Fixture::new(2)?;
    let app = &mut fixture.app;
    assert_eq!(app.actions(), [DetailAction::Implement]);
    app.store.update_settings(|settings| {
        settings.dev_skill = Some("/team-dev".into());
        Ok(())
    })?;
    app.refresh()?;
    assert_eq!(
        app.actions(),
        [DetailAction::StartDev, DetailAction::Implement]
    );
    press(app, KeyCode::Enter)?;
    assert!(app.notice.text().contains("inside Herdr to start sessions"));
    assert_eq!(app.current().unwrap().implementation.status, "ready");
    Ok(())
}

#[test]
fn a_new_spec_takes_the_client_first_and_then_an_optional_topic() -> Result<()> {
    let mut fixture = Fixture::new(1)?;
    let app = &mut fixture.app;
    press(app, KeyCode::Esc)?;
    app.choose_client(ChoicePurpose::NewSpec, vec![Profile::Opus, Profile::Codex]);
    let choice = |app: &App| {
        let choice = app.modal.choice.as_ref().unwrap();
        (choice.selected, choice.entering_topic)
    };
    // Step one: letters move the selection; nothing is typed yet.
    press(app, KeyCode::Char('j'))?;
    assert_eq!(choice(app), (1, false));
    assert!(app.modal.input.is_empty());
    press(app, KeyCode::Enter)?;
    assert_eq!(choice(app), (1, true));

    // Step two: every key types, and Esc steps back without losing the client or the text.
    type_text(app, "jk retries")?;
    assert_eq!(app.modal.input, "jk retries");
    assert_eq!(choice(app), (1, true));
    press(app, KeyCode::Esc)?;
    assert_eq!(choice(app), (1, false));
    press(app, KeyCode::Char('k'))?;
    press(app, KeyCode::Enter)?;
    assert_eq!(choice(app), (0, true));
    assert_eq!(app.modal.input, "jk retries");

    // Starting fails here because tests never reach Herdr; everything stays for a retry.
    press(app, KeyCode::Enter)?;
    assert!(app.notice.text().contains("inside Herdr"));
    assert_eq!(choice(app), (0, true));
    assert_eq!(app.modal.input, "jk retries");
    assert!(app.records.iter().all(|record| !record.title.is_empty()));

    // Esc from the first step cancels the whole thing.
    press(app, KeyCode::Esc)?;
    press(app, KeyCode::Esc)?;
    assert!(!app.modal.is_open());
    assert!(app.modal.input.is_empty());
    Ok(())
}
