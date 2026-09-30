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
