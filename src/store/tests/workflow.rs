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
    let (deleted, moved) = store.delete_archived(&gone.id)?;
    assert_eq!((deleted.id, moved), (gone.id.clone(), true));
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
    assert!(!store.delete_archived(&kept.id)?.1);
    assert!(store.trash_list()?.is_empty());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn relinking_to_a_scanned_copy_replaces_the_placeholder_a_scan_imported() -> Result<()> {
    let root = temp_root("relink-rename");
    let store = configured_store(&root)?;
    let source = root.join("selected");
    fs::write(source.join("one.md"), "# One")?;
    store.scan()?;
    let original = store.list()?.remove(0);
    let jira = |key: &str| Change::Jira {
        key: key.into(),
        url: None,
    };
    store.update(&original.id, jira("ONE-1"))?;

    // The spec continues in a new file that a scan has already imported as a fresh item.
    fs::copy(source.join("one.md"), source.join("renamed.md"))?;
    assert_eq!(store.scan()?.imported, 1);
    assert_eq!(store.list()?.len(), 2);

    let relinked = store.relink(&original.id, source.join("renamed.md"))?;
    assert_eq!(relinked.id, original.id);
    assert_eq!(relinked.jira.key.as_deref(), Some("ONE-1"));
    let records = store.list()?;
    assert_eq!(records.len(), 1, "the placeholder import is gone");
    assert_eq!(records[0].id, original.id);
    fs::remove_file(source.join("one.md"))?;
    let report = store.scan()?;
    assert_eq!((report.imported, report.known, report.dropped), (0, 1, 0));
    assert!(report.issues.is_empty(), "{:?}", report.issues);

    // An item someone has worked on is never replaced.
    fs::write(source.join("other.md"), "# Other")?;
    store.scan()?;
    let other = store
        .list()?
        .into_iter()
        .find(|record| record.id != original.id)
        .unwrap();
    store.update(&other.id, jira("OTHER-1"))?;
    assert!(store.relink(&original.id, source.join("other.md")).is_err());
    assert_eq!(store.get(&other.id)?.jira.key.as_deref(), Some("OTHER-1"));
    // Neither is an archived one.
    store.archive(&other.id)?;
    assert!(store.relink(&original.id, source.join("other.md")).is_err());
    assert_eq!(store.trash_list()?.len(), 1);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn scans_stay_quiet_about_unwritten_new_specs_and_unselected_folders() -> Result<()> {
    let root = temp_root("scan-quiet");
    let store = configured_store(&root)?;
    let source = root.join("selected");
    fs::write(source.join("kept.md"), "# Kept")?;
    store.scan()?;

    // A new spec session is recorded before its agent has written the file.
    let pending = store.start_untitled(None)?;
    assert!(!pending.spec_path.exists());
    let launch = Launch {
        status: LaunchStatus::PromptSent,
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
    store.update(
        &pending.id,
        Change::Launch(Box::new(launch), pending.spec_path.clone()),
    )?;
    assert!(store.scan()?.issues.is_empty());

    // Once the session is settled, a spec that was never written is dropped like a deleted one.
    store.settle(&pending.id)?;
    let report = store.scan()?;
    assert_eq!(report.dropped, 1);
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    assert!(store.get(&pending.id).is_err());

    // After another folder is selected, the old folder's records are out of scope.
    let other = root.join("other");
    fs::create_dir_all(&other)?;
    let mut settings = store.settings()?;
    settings.sources = vec![SpecSource::new(other)?];
    store.save_settings(&settings)?;
    assert!(store.scan()?.issues.is_empty());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn scan_drops_records_whose_file_was_deleted_from_the_selected_folder() -> Result<()> {
    let root = temp_root("drop-missing");
    let store = configured_store(&root)?;
    let source = root.join("selected");
    fs::write(source.join("kept.md"), "# Kept")?;
    fs::write(source.join("deleted.md"), "# Deleted")?;
    store.scan()?;
    let deleted = store
        .list()?
        .into_iter()
        .find(|record| record.title == "Deleted")
        .unwrap();
    let jira = Change::Jira {
        key: "GONE-1".into(),
        url: None,
    };
    store.update(&deleted.id, jira)?;

    fs::remove_file(&deleted.spec_path)?;
    let report = store.scan()?;
    assert_eq!((report.dropped, report.known, report.imported), (1, 1, 0));
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    assert!(store.get(&deleted.id).is_err());
    assert_eq!(store.list()?.len(), 1);
    assert_eq!(store.scan()?.dropped, 0);

    // A folder that cannot be read right now proves nothing about its files.
    let kept = store.list()?.remove(0);
    fs::rename(&source, root.join("unmounted"))?;
    let report = store.scan()?;
    assert_eq!(report.dropped, 0);
    assert!(!report.issues.is_empty());
    assert_eq!(store.get(&kept.id)?.id, kept.id);
    fs::rename(root.join("unmounted"), &source)?;
    assert_eq!(store.scan()?.known, 1);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn archive_holds_only_existing_specs_from_the_selected_folder() -> Result<()> {
    let root = temp_root("archive-scope");
    let store = configured_store(&root)?;
    let source = root.join("selected");
    for name in ["stays", "deleted", "active"] {
        fs::write(source.join(format!("{name}.md")), format!("# {name}"))?;
    }
    store.scan()?;
    let by_title = |title: &str| -> Result<Record> {
        Ok(store
            .all_records()?
            .into_iter()
            .find(|record| record.title == title)
            .ok_or("missing fixture record")?)
    };
    let (stays, deleted, active) = (
        by_title("stays")?,
        by_title("deleted")?,
        by_title("active")?,
    );
    store.archive(&stays.id)?;
    store.archive(&deleted.id)?;
    assert_eq!(store.scan()?.suppressed, 2);

    // An archived spec whose file is gone has nothing left to restore.
    fs::remove_file(&deleted.spec_path)?;
    let report = store.scan()?;
    assert_eq!((report.dropped, report.suppressed), (1, 1));
    assert_eq!(store.trash_list()?.len(), 1);
    assert!(store.restore(&deleted.id).is_err());

    // Selecting another folder empties the archive of the old folder's specs. Their files
    // are untouched, and the old folder's active items keep their records.
    let other = root.join("other");
    fs::create_dir_all(&other)?;
    let mut settings = store.settings()?;
    settings.sources = vec![SpecSource::new(other)?];
    store.save_settings(&settings)?;
    let report = store.scan()?;
    assert_eq!(report.dropped, 1);
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    assert!(store.trash_list()?.is_empty());
    assert!(stays.spec_path.is_file());
    assert_eq!(store.get(&active.id)?.id, active.id);
    fs::remove_dir_all(root)?;
    Ok(())
}
