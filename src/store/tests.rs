use super::*;

#[test]
fn spec_to_pr_flow_requires_jira_before_implementation() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-test-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    let started = store.start("Payment retries", None, None)?;
    assert!(started.spec_path.is_file());
    assert_eq!(started.next_actions(), vec!["Finish spec"]);
    assert!(
        store
            .update(
                &started.id,
                Change::Implement {
                    agent: None,
                    branch: None
                }
            )
            .is_err()
    );
    assert!(
        store
            .update(
                &started.id,
                Change::Jira {
                    key: "ABC-123".into(),
                    url: None,
                }
            )
            .is_err()
    );

    let finished = store.update(&started.id, Change::Finish { title: None })?;
    assert_eq!(finished.next_actions(), vec!["Create Jira ticket"]);
    assert_eq!(finished.implementation_stage(), "locked");
    assert_eq!(finished.pr_stage(), "locked");
    assert!(
        store
            .update(
                &started.id,
                Change::Jira {
                    key: "  ".into(),
                    url: None,
                },
            )
            .is_err()
    );
    assert!(
        store
            .update(
                &started.id,
                Change::Implement {
                    agent: Some("codex".into()),
                    branch: Some("feature/payment-retries".into()),
                },
            )
            .is_err()
    );
    assert!(
        store
            .update(
                &started.id,
                Change::Pr {
                    url: "https://example.test/pr/1".into(),
                },
            )
            .is_err()
    );
    let linked = store.update(
        &started.id,
        Change::Jira {
            key: "ABC-123".into(),
            url: None,
        },
    )?;
    assert_eq!(linked.next_actions(), vec!["Hand spec to implementor"]);
    assert_eq!(linked.implementation_stage(), "ready");
    assert_eq!(linked.pr_stage(), "locked");
    assert!(
        store
            .update(
                &started.id,
                Change::Pr {
                    url: "https://example.test/pr/1".into(),
                },
            )
            .is_err()
    );
    let implementing = store.update(
        &started.id,
        Change::Implement {
            agent: Some("codex".into()),
            branch: Some("feature/payment-retries".into()),
        },
    )?;
    assert_eq!(implementing.implementation.status, "in_progress");
    assert_eq!(implementing.pr_stage(), "ready");
    assert_eq!(implementing.next_actions(), vec!["Await draft PR"]);
    let pr = store.update(
        &started.id,
        Change::Pr {
            url: "https://example.test/pr/1".into(),
        },
    )?;
    assert_eq!(pr.next_actions(), vec!["Review draft PR"]);
    assert_eq!(store.list()?.len(), 1);
    assert_eq!(fs::read_dir(root.join("items"))?.count(), 1);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn legacy_active_implementation_keeps_progress_but_requires_jira_for_new_work() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-legacy-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    let started = store.start("Legacy flow", None, None)?;
    let mut legacy = store.update(&started.id, Change::Finish { title: None })?;
    legacy.implementation.status = "in_progress".into();
    store.write(&legacy)?;
    assert_eq!(
        store.get(&started.id)?.implementation_stage(),
        "in_progress"
    );
    assert_eq!(store.get(&started.id)?.pr_stage(), "locked");
    assert!(
        store
            .update(
                &started.id,
                Change::Pr {
                    url: "https://example.test/pr/2".into(),
                },
            )
            .is_err()
    );
    let linked = store.update(
        &started.id,
        Change::Jira {
            key: "ABC-456".into(),
            url: None,
        },
    )?;
    assert_eq!(linked.pr_stage(), "ready");
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn existing_spec_is_preserved() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&root)?;
    let spec = root.join("existing.md");
    fs::write(&spec, "Existing work\n")?;
    let store = Store::new(root.clone());
    let record = store.start("Existing", None, Some(spec.clone()))?;
    assert_eq!(fs::read_to_string(&spec)?, "Existing work\n");
    assert_eq!(record.spec_path, spec);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn untitled_session_waits_for_written_spec_and_final_title() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-test-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    let record = store.start_untitled(None)?;
    assert_eq!(record.display_title(), "Untitled spec");
    assert!(!record.spec_path.exists());
    assert!(
        store
            .update(
                &record.id,
                Change::Finish {
                    title: Some("Final name".into())
                }
            )
            .is_err()
    );
    fs::write(&record.spec_path, "# Final name\n\nReal spec\n")?;
    let done = store.update(
        &record.id,
        Change::Finish {
            title: Some("Final name".into()),
        },
    )?;
    assert_eq!(done.title, "Final name");
    assert_eq!(done.spec, "done");
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn deleting_item_moves_only_inbox_owned_spec_to_trash() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-test-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    let owned = store.start("Owned", None, None)?;
    let external_path = root.join("elsewhere.md");
    fs::write(&external_path, "Keep me")?;
    let external = store.start("Linked", None, Some(external_path.clone()))?;

    store.delete(&owned.id)?;
    store.delete(&external.id)?;
    assert!(store.list()?.is_empty());
    assert!(!owned.spec_path.exists());
    assert!(
        root.join("trash/specs")
            .join(format!("{}.md", owned.id))
            .is_file()
    );
    assert!(
        root.join("trash/items")
            .join(format!("{}.json", owned.id))
            .is_file()
    );
    assert!(
        root.join("trash/items")
            .join(format!("{}.json", external.id))
            .is_file()
    );
    assert_eq!(fs::read_to_string(&external_path)?, "Keep me");
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn refining_and_finishing_preserves_links_and_launch_history() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-refine-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    let record = store.start("Existing title", None, None)?;
    store.update(&record.id, Change::Finish { title: None })?;
    store.update(
        &record.id,
        Change::Jira {
            key: "PAY-123".into(),
            url: Some("https://jira.test/PAY-123".into()),
        },
    )?;
    store.update(
        &record.id,
        Change::Implement {
            agent: Some("implementor".into()),
            branch: Some("feature/payment".into()),
        },
    )?;
    store.update(
        &record.id,
        Change::Pr {
            url: "https://git.test/pr/1".into(),
        },
    )?;
    let launch = Launch {
        status: "prompt_sent".into(),
        harness: "codex".into(),
        workspace: "ai-boiler-room".into(),
        workspace_id: Some("w1".into()),
        tab_id: Some("t1".into()),
        pane_id: Some("p1".into()),
        agent: Some("spec_original".into()),
        model: "gpt-6-sol".into(),
        effort: "high".into(),
        prompt: "original prompt".into(),
        error: None,
    };
    store.update(&record.id, Change::Launch(Box::new(launch.clone())))?;
    let mut refinement = launch;
    refinement.status = "starting".into();
    refinement.agent = Some("refine_new".into());
    let starting = store.update(
        &record.id,
        Change::BeginRefinement(Box::new(refinement.clone())),
    )?;
    assert_eq!(starting.spec, "done");
    assert_eq!(starting.previous_launches.len(), 1);
    assert_eq!(
        starting.previous_launches[0].agent.as_deref(),
        Some("spec_original")
    );
    refinement.status = "prompt_sent".into();
    store.update(&record.id, Change::Launch(Box::new(refinement)))?;
    let refining = store.update(&record.id, Change::RefineSpec)?;
    assert_eq!(refining.spec, "in_progress");
    let finished = store.update(&record.id, Change::Finish { title: None })?;
    assert_eq!(finished.title, "Existing title");
    assert_eq!(finished.spec_path, record.spec_path);
    assert_eq!(finished.jira.status, "created");
    assert_eq!(finished.jira.key.as_deref(), Some("PAY-123"));
    assert_eq!(
        finished.jira.url.as_deref(),
        Some("https://jira.test/PAY-123")
    );
    assert_eq!(finished.implementation.status, "draft_pr");
    assert_eq!(
        finished.implementation.agent.as_deref(),
        Some("implementor")
    );
    assert_eq!(
        finished.implementation.branch.as_deref(),
        Some("feature/payment")
    );
    assert_eq!(finished.pr.status, "draft");
    assert_eq!(finished.pr.url.as_deref(), Some("https://git.test/pr/1"));
    assert_eq!(finished.previous_launches.len(), 1);
    assert_eq!(store.list()?.len(), 1);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn legacy_records_without_launch_history_still_load() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-history-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    let record = store.start("Legacy", None, None)?;
    let mut json = serde_json::to_value(&record)?;
    json.as_object_mut().unwrap().remove("previous_launches");
    fs::write(store.path_for(&record.id)?, serde_json::to_vec(&json)?)?;
    assert!(store.get(&record.id)?.previous_launches.is_empty());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn refinement_pauses_new_progression_without_erasing_active_work() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-refine-locks-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    let record = store.start("Existing", None, None)?;
    store.update(&record.id, Change::Finish { title: None })?;
    let linked = store.update(
        &record.id,
        Change::Jira {
            key: "PAY-1".into(),
            url: None,
        },
    )?;
    assert_eq!(linked.implementation_stage(), "ready");
    let refining = store.update(&record.id, Change::RefineSpec)?;
    assert_eq!(refining.implementation_stage(), "locked");
    store.update(&record.id, Change::Finish { title: None })?;
    let active = store.update(
        &record.id,
        Change::Implement {
            agent: None,
            branch: Some("feature/existing".into()),
        },
    )?;
    assert_eq!(active.pr_stage(), "ready");
    let refining = store.update(&record.id, Change::RefineSpec)?;
    assert_eq!(refining.implementation_stage(), "in_progress");
    assert_eq!(refining.pr_stage(), "locked");
    assert_eq!(
        refining.implementation.branch.as_deref(),
        Some("feature/existing")
    );
    let finished = store.update(&record.id, Change::Finish { title: None })?;
    assert_eq!(finished.pr_stage(), "ready");
    fs::remove_dir_all(root)?;
    Ok(())
}
