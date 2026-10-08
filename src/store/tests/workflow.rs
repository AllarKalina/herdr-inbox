use super::*;

fn temp_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("herdr-{name}-{}", Uuid::new_v4()))
}

#[test]
fn turning_jira_off_unlocks_development_after_the_spec_and_keeps_stored_progress() -> Result<()> {
    let root = temp_root("jira-off");
    let store = configured_store(&root)?;
    let id = store.start("No ticket needed", None, None)?.id;
    let mut settings = store.settings()?;
    assert!(settings.jira);
    settings.jira = false;
    store.save_settings(&settings)?;
    let implement = || Change::Implement {
        agent: None,
        branch: Some("feature/no-ticket".into()),
    };
    let error = store.update(&id, implement()).unwrap_err().to_string();
    assert_eq!(error, "Finish the spec before implementation");
    let finished = store.update(&id, Change::Finish { title: None })?;
    assert_eq!(finished.implementation_stage(true), "locked");
    assert_eq!(finished.implementation_stage(false), "ready");
    assert_eq!(finished.pr_stage(false), "locked");
    assert_eq!(
        finished.next_actions(false),
        vec!["Hand spec to implementor"]
    );
    let jira = || Change::Jira {
        key: "OFF-1".into(),
        url: None,
    };
    let error = store.update(&id, jira()).unwrap_err().to_string();
    assert_eq!(error, "Jira is turned off in Settings");
    let implementing = store.update(&id, implement())?;
    assert_eq!(implementing.pr_stage(false), "ready");
    let url = "https://github.example/org/repo/pull/7".to_string();
    let drafted = store.update(&id, Change::Pr { url })?;
    assert_eq!(drafted.pr.status, "draft");

    // Turning Jira back on keeps the recorded work and shows the ticket as outstanding.
    settings.jira = true;
    store.save_settings(&settings)?;
    let kept = store.get(&id)?;
    assert_eq!(kept.jira.status, "ready");
    assert_eq!(kept.implementation.status, "draft_pr");
    assert_eq!(kept.pr_stage(true), "draft");
    assert_eq!(store.update(&id, jira())?.jira.status, "created");
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn deleting_an_archived_spec_trashes_its_file_and_removes_the_record() -> Result<()> {
    let root = temp_root("archive-delete");
    let store = configured_store(&root)?;
    let source = root.join("selected");
    fs::write(source.join("gone.md"), "# Gone")?;
    fs::write(source.join("kept.md"), "# Kept")?;
    store.scan()?;
    let records = store.list()?;
    let gone = records.iter().find(|r| r.title == "Gone").unwrap().clone();
    assert!(
        store.delete_archived(&gone.id).is_err(),
        "only archived specs can be deleted"
    );
    assert!(gone.spec_path.is_file());
    store.archive(&gone.id)?;
    assert_eq!(store.delete_archived(&gone.id)?.id, gone.id);
    assert!(!gone.spec_path.exists());
    assert_eq!(
        fs::read_to_string(root.join("test-macos-trash/gone.md"))?,
        "# Gone"
    );
    assert!(store.trash_list()?.is_empty());
    assert!(store.restore(&gone.id).is_err());
    let report = store.scan()?;
    assert_eq!(
        (report.imported, report.known, report.suppressed),
        (0, 1, 0)
    );

    // A spec whose file is already gone only loses its archived record.
    let kept = store.list()?.remove(0);
    store.archive(&kept.id)?;
    fs::remove_file(&kept.spec_path)?;
    store.delete_archived(&kept.id)?;
    assert!(store.trash_list()?.is_empty());
    fs::remove_dir_all(root)?;
    Ok(())
}
