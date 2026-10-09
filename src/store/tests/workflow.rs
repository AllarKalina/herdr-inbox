use super::*;

/// A finished spec with nothing downstream: where every imported spec starts.
fn imported() -> Record {
    let location = Location {
        source_id: "source".into(),
        relative: "spec.md".into(),
        path: "/nowhere/spec.md".into(),
    };
    Record::new("id".into(), location, SpecStatus::Done, 0)
}

fn jira_link() -> Change {
    Change::Jira {
        key: "ABC-1".into(),
        url: None,
    }
}

fn implement() -> Change {
    Change::Implement {
        agent: None,
        branch: Some("feature/x".into()),
    }
}

fn draft_pr() -> Change {
    Change::Pr {
        url: "https://example.test/pull/1".into(),
    }
}

#[test]
fn each_step_unlocks_the_next_and_names_what_to_do() -> Result<()> {
    let mut record = imported();
    assert_eq!(record.next_actions(true), ["Create Jira ticket"]);
    assert_eq!(record.implementation_stage(true), "locked");
    assert!(record.apply(implement(), true).is_err());
    assert!(record.apply(draft_pr(), true).is_err());
    let blank = Change::Jira {
        key: "  ".into(),
        url: None,
    };
    assert!(record.apply(blank, true).is_err());

    record.apply(jira_link(), true)?;
    assert_eq!(record.next_actions(true), ["Hand spec to implementor"]);
    assert_eq!(record.implementation_stage(true), "ready");
    assert_eq!(record.pr_stage(true), "locked");
    assert!(record.apply(draft_pr(), true).is_err());

    record.apply(implement(), true)?;
    assert_eq!(record.next_actions(true), ["Await draft PR"]);
    assert_eq!(record.pr_stage(true), "ready");

    record.apply(draft_pr(), true)?;
    assert_eq!(record.next_actions(true), ["Review draft PR"]);
    assert_eq!(record.implementation_stage(true), "draft_pr");
    assert_eq!(record.pr_stage(true), "draft");
    // Recording the implementer again never takes a draft PR back.
    record.apply(implement(), true)?;
    assert_eq!(record.implementation.status, "draft_pr");
    Ok(())
}

#[test]
fn without_jira_a_finished_spec_is_the_only_prerequisite() -> Result<()> {
    let mut record = imported();
    assert_eq!(record.implementation_stage(false), "ready");
    assert_eq!(record.next_actions(false), ["Hand spec to implementor"]);
    let refused = record.apply(jira_link(), false).unwrap_err();
    assert_eq!(refused.to_string(), "Jira is turned off in Settings");
    record.apply(implement(), false)?;
    record.apply(draft_pr(), false)?;
    // Turning Jira on later shows the ticket as outstanding and keeps the progress.
    assert_eq!(record.jira.status, "ready");
    assert_eq!(record.pr_stage(true), "draft");
    record.apply(jira_link(), true)?;
    assert_eq!(record.jira.status, "created");

    let mut unfinished = imported();
    unfinished.spec = SpecStatus::InProgress;
    let refused = unfinished.apply(implement(), false).unwrap_err();
    assert_eq!(refused.to_string(), "Finish the spec before implementation");
    assert_eq!(unfinished.next_actions(false), ["Finish spec"]);
    Ok(())
}

fn temp_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("herdr-{name}-{}", Uuid::new_v4()))
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
    assert!(store.archived()?.is_empty());
    assert!(store.restore(&gone.id).is_err());
    let report = store.scan()?;
    assert_eq!((report.imported, report.known, report.archived), (0, 1, 0));

    // A spec whose file is already gone only loses its archived record.
    let kept = store.list()?.remove(0);
    store.archive(&kept.id)?;
    fs::remove_file(&kept.spec_path)?;
    assert!(!store.delete_archived(&kept.id)?.1);
    assert!(store.archived()?.is_empty());
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
    let original = store.update(&original.id, jira("ONE-1"))?;

    // The spec continues in a new file that a scan has already imported as a fresh item.
    fs::copy(&original.spec_path, source.join("renamed.md"))?;
    assert_eq!(store.scan()?.imported, 1);
    assert_eq!(store.list()?.len(), 2);

    let relinked = store.relink(&original.id, source.join("renamed.md"))?;
    assert_eq!(relinked.id, original.id);
    assert_eq!(relinked.jira.key.as_deref(), Some("ONE-1"));
    let records = store.list()?;
    assert_eq!(records.len(), 1, "the placeholder import is gone");
    assert_eq!(records[0].id, original.id);
    fs::remove_file(&original.spec_path)?;
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
    assert_eq!(store.archived()?.len(), 1);
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
    let pending = store.start_untitled(None, None)?;
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
    let deleted = store.update(&deleted.id, jira)?;

    fs::remove_file(&deleted.spec_path)?;
    let report = store.scan()?;
    assert_eq!((report.dropped, report.known, report.imported), (1, 1, 0));
    assert!(report.issues.is_empty(), "{:?}", report.issues);

    // A file that cannot be read as text is reported and never imported.
    fs::write(source.join("binary.md"), [0xff, 0xfe])?;
    let report = store.scan()?;
    assert_eq!(report.imported, 0);
    assert!(
        report.issues[0].contains("Cannot read spec"),
        "{:?}",
        report.issues
    );
    fs::remove_file(source.join("binary.md"))?;
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
    assert_eq!(store.scan()?.archived, 2);

    // An archived spec whose file is gone has nothing left to restore.
    fs::remove_file(&deleted.spec_path)?;
    let report = store.scan()?;
    assert_eq!((report.dropped, report.archived), (1, 1));
    assert_eq!(store.archived()?.len(), 1);
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
    assert!(store.archived()?.is_empty());
    assert!(stays.spec_path.is_file());
    assert_eq!(store.get(&active.id)?.id, active.id);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn a_requested_ticket_waits_for_its_key_and_never_unlocks_development() -> Result<()> {
    let mut record = imported();
    assert!(record.permits(Change::RequestJira, false).is_err());
    record.apply(Change::RequestJira, true)?;
    assert_eq!(record.jira.status, "requested");
    assert_eq!(record.next_actions(true), ["Await Jira ticket"]);
    assert_eq!(record.implementation_stage(true), "locked");
    assert!(!record.untouched_import());
    // The agent reports back, or the user links one by hand: either completes the stage.
    record.apply(jira_link(), true)?;
    assert_eq!(record.jira.status, "created");
    // Asking again for a linked item changes nothing.
    record.apply(Change::RequestJira, true)?;
    assert_eq!(record.jira.status, "created");

    let mut unfinished = imported();
    unfinished.spec = SpecStatus::InProgress;
    assert!(unfinished.permits(Change::RequestJira, true).is_err());
    Ok(())
}

#[test]
fn linking_a_ticket_names_the_spec_file_after_its_key() -> Result<()> {
    let root = temp_root("ticket-name");
    let store = configured_store(&root)?;
    let source = root.join("selected");
    fs::create_dir_all(source.join("payments"))?;
    fs::write(
        source.join("payments/notes.markdown"),
        "# Payment retries: v2 (EU & UK)\n",
    )?;
    fs::write(source.join("plain.md"), "No heading here\n")?;
    store.scan()?;
    let by_title = |title: &str| store.list().unwrap().into_iter().find(|r| r.title == title);
    let link = |key: &str| Change::Jira {
        key: key.into(),
        url: None,
    };

    let retries = by_title("Payment retries: v2 (EU & UK)").unwrap();
    let linked = store.update(&retries.id, link("BT-2300"))?;
    let renamed =
        fs::canonicalize(&source)?.join("payments/BT-2300-payment-retries-v2-eu-uk.markdown");
    assert_eq!(linked.spec_path, renamed);
    assert_eq!(
        linked.source_relative_path,
        PathBuf::from("payments/BT-2300-payment-retries-v2-eu-uk.markdown")
    );
    assert!(renamed.is_file());
    assert!(!source.join("payments/notes.markdown").exists());
    // The item keeps its identity: a scan neither imports the new name nor drops the old.
    let report = store.scan()?;
    assert_eq!((report.imported, report.dropped, report.known), (0, 0, 2));

    // Linking the same key again leaves the name alone; a new key replaces the old prefix.
    assert_eq!(
        store.update(&retries.id, link("BT-2300"))?.spec_path,
        renamed
    );
    let moved = store.update(&retries.id, link("BT-2400"))?;
    assert!(
        moved
            .spec_path
            .ends_with("payments/BT-2400-payment-retries-v2-eu-uk.markdown")
    );
    assert!(!renamed.exists());

    // A name that is already taken refuses the link and changes nothing.
    let plain = by_title("plain").unwrap();
    fs::write(source.join("BT-7-plain.md"), "Someone else's file\n")?;
    let refused = store
        .update(&plain.id, link("BT-7"))
        .unwrap_err()
        .to_string();
    assert!(refused.contains("already exists"), "{refused}");
    let untouched = store.get(&plain.id)?;
    assert_eq!(untouched.jira.status, "ready");
    assert!(untouched.spec_path.ends_with("plain.md"));
    assert!(source.join("plain.md").is_file());
    fs::remove_dir_all(root)?;
    Ok(())
}
