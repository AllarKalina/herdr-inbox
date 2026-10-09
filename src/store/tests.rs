use super::*;

fn configured_store(root: &Path) -> Result<Store> {
    let source = root.join("selected");
    fs::create_dir_all(&source)?;
    let store = Store::new(root.to_path_buf());
    let mut settings = Settings::default();
    settings.sources.push(SpecSource::new(source)?);
    store.save_settings(&settings)?;
    Ok(store)
}

#[test]
fn existing_spec_is_preserved() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-test-{}", Uuid::new_v4()));
    fs::create_dir_all(root.join("selected"))?;
    let spec = root.join("selected/existing.md");
    fs::write(&spec, "Existing work\n")?;
    let store = configured_store(&root)?;
    let record = store.start("Existing", None, Some(spec.clone()))?;
    assert_eq!(fs::read_to_string(&spec)?, "Existing work\n");
    assert_eq!(record.spec_path, fs::canonicalize(spec)?);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn untitled_session_waits_for_written_spec_and_final_title() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-test-{}", Uuid::new_v4()));
    let store = configured_store(&root)?;
    let record = store.start_untitled(None, None)?;
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
fn archiving_only_moves_metadata_and_keeps_all_source_files() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-test-{}", Uuid::new_v4()));
    let store = configured_store(&root)?;
    let owned = store.start("Created", None, None)?;
    let external_path = root.join("selected/elsewhere.md");
    fs::write(&external_path, "Keep me")?;
    let external = store.start("Linked", None, Some(external_path.clone()))?;

    store.archive(&owned.id)?;
    store.archive(&external.id)?;
    assert!(store.list()?.is_empty());
    assert!(owned.spec_path.exists());
    assert!(!root.join("trash/specs").exists());
    assert!(
        root.join("archive")
            .join(format!("{}.json", owned.id))
            .is_file()
    );
    assert!(
        root.join("archive")
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
    let store = configured_store(&root)?;
    let record = store.start("Existing title", None, None)?;
    store.update(&record.id, Change::Finish { title: None })?;
    store.update(
        &record.id,
        Change::Jira {
            key: "PAY-123".into(),
            url: Some("https://jira.test/PAY-123".into()),
        },
    )?;
    // Linking renamed the spec file; later steps must use the item's current path.
    let record = store.get(&record.id)?;
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
        status: LaunchStatus::Completed,
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
    store.update(
        &record.id,
        Change::Launch(Box::new(launch.clone()), record.spec_path.clone()),
    )?;
    let mut refinement = launch;
    refinement.status = LaunchStatus::Starting;
    refinement.agent = Some("refine_new".into());
    let starting = store.update(
        &record.id,
        Change::BeginRefinement(Box::new(refinement.clone()), record.spec_path.clone()),
    )?;
    assert_eq!(starting.spec, "done");
    assert_eq!(starting.previous_launches.len(), 1);
    assert_eq!(
        starting.previous_launches[0].agent.as_deref(),
        Some("spec_original")
    );
    refinement.status = LaunchStatus::PromptSent;
    store.update(
        &record.id,
        Change::Launch(Box::new(refinement), record.spec_path.clone()),
    )?;
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
fn refinement_pauses_new_progression_without_erasing_active_work() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-inbox-refine-locks-{}", Uuid::new_v4()));
    let store = configured_store(&root)?;
    let record = store.start("Existing", None, None)?;
    store.update(&record.id, Change::Finish { title: None })?;
    let linked = store.update(
        &record.id,
        Change::Jira {
            key: "PAY-1".into(),
            url: None,
        },
    )?;
    assert_eq!(linked.implementation_stage(true), "ready");
    let refining = store.update(&record.id, Change::RefineSpec)?;
    assert_eq!(refining.implementation_stage(true), "locked");
    store.update(&record.id, Change::Finish { title: None })?;
    let active = store.update(
        &record.id,
        Change::Implement {
            agent: None,
            branch: Some("feature/existing".into()),
        },
    )?;
    assert_eq!(active.pr_stage(true), "ready");
    let refining = store.update(&record.id, Change::RefineSpec)?;
    assert_eq!(refining.implementation_stage(true), "in_progress");
    assert_eq!(refining.pr_stage(true), "locked");
    assert_eq!(
        refining.implementation.branch.as_deref(),
        Some("feature/existing")
    );
    let finished = store.update(&record.id, Change::Finish { title: None })?;
    assert_eq!(finished.pr_stage(true), "ready");
    fs::remove_dir_all(root)?;
    Ok(())
}

mod discovery;

mod settings;
mod workflow;
