use super::*;
use std::os::unix::fs::symlink;

fn fixture() -> Result<(PathBuf, Store, PathBuf)> {
    let root = std::env::temp_dir().join(format!("herdr-sources-{}", Uuid::new_v4()));
    let source = root.join("arbitrary folder");
    fs::create_dir_all(source.join("nested"))?;
    let store = Store::new(root.join("app"));
    let mut settings = store.settings()?;
    settings.sources.push(SpecSource::new(source.clone())?);
    store.save_settings(&settings)?;
    Ok((root, store, source))
}

#[test]
fn imports_nested_done_and_preserves_enrichment_on_repeat() -> Result<()> {
    let (root, store, source) = fixture()?;
    fs::write(
        source.join("nested/anything.markdown"),
        "preamble\n# My spec\n\nDetails",
    )?;
    fs::write(source.join("plain.md"), "No heading")?;
    fs::write(source.join("ignored.txt"), "# Ignore")?;
    assert_eq!(store.scan()?.imported, 2);
    let record = store
        .list()?
        .into_iter()
        .find(|r| r.title == "My spec")
        .unwrap();
    assert_eq!(record.spec, "done");
    assert_eq!(record.jira.status, "ready");
    assert_eq!(record.implementation_stage(), "locked");
    assert_eq!(record.pr_stage(), "locked");
    assert!(!store.manages_spec(&record));
    assert_eq!(
        record.source_relative_path,
        Some(PathBuf::from("nested/anything.markdown"))
    );
    store.update(
        &record.id,
        Change::Jira {
            key: "TEST-1".into(),
            url: None,
        },
    )?;
    store.update(
        &record.id,
        Change::Implement {
            agent: Some("codex".into()),
            branch: Some("test".into()),
        },
    )?;
    let enriched = store.update(
        &record.id,
        Change::Pr {
            url: "https://example.test/1".into(),
        },
    )?;
    fs::write(&record.spec_path, "# Changed content")?;
    let report = store.scan()?;
    assert_eq!((report.imported, report.known), (0, 2));
    assert_eq!(
        serde_json::to_value(store.get(&record.id)?)?,
        serde_json::to_value(enriched)?
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn overlaps_aliases_and_cycles_are_deduplicated() -> Result<()> {
    let (root, store, source) = fixture()?;
    fs::write(source.join("nested/one.md"), "# One")?;
    symlink(source.join("nested/one.md"), source.join("alias.md"))?;
    symlink(&source, source.join("nested/cycle"))?;
    let mut settings = store.settings()?;
    settings
        .sources
        .push(SpecSource::new(source.join("nested"))?);
    store.save_settings(&settings)?;
    assert_eq!(store.scan()?.imported, 1);
    assert_eq!(store.list()?.len(), 1);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn missing_and_invalid_specs_keep_metadata_with_errors() -> Result<()> {
    let (root, store, source) = fixture()?;
    fs::write(source.join("one.md"), "# One")?;
    store.scan()?;
    let record = store.list()?.remove(0);
    fs::remove_file(source.join("one.md"))?;
    fs::write(source.join("invalid.md"), [0xff, 0xfe])?;
    let report = store.scan()?;
    assert!(report.issues.iter().any(|s| s.contains("Cannot read spec")));
    assert!(report.issues.iter().any(|s| s.contains("Spec unavailable")));
    fs::rename(&source, root.join("gone"))?;
    assert!(
        store
            .scan()?
            .issues
            .iter()
            .any(|s| s.contains("Source unavailable"))
    );
    assert_eq!(store.get(&record.id)?.id, record.id);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn deletion_suppresses_reimport_and_restore_keeps_identity() -> Result<()> {
    let (root, store, source) = fixture()?;
    fs::write(source.join("spec.md"), "# Spec")?;
    store.scan()?;
    let record = store.list()?.remove(0);
    let enriched = store.update(
        &record.id,
        Change::Jira {
            key: "A-1".into(),
            url: None,
        },
    )?;
    store.delete(&record.id)?;
    assert!(record.spec_path.is_file());
    assert_eq!(store.scan()?.suppressed, 1);
    assert!(store.list()?.is_empty());
    assert_eq!(
        serde_json::to_value(store.restore(&record.id)?)?,
        serde_json::to_value(enriched)?
    );
    assert_eq!(store.scan()?.known, 1);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn relocation_matches_content_and_unrelated_source_gets_new_identity() -> Result<()> {
    let (root, store, source) = fixture()?;
    fs::write(source.join("one.md"), "# One")?;
    fs::write(source.join("unmatched.md"), "# Original")?;
    store.scan()?;
    let records = store.list()?;
    let original = records.iter().find(|r| r.title == "One").unwrap();
    let unmatched = records.iter().find(|r| r.title == "Original").unwrap();
    let moved = root.join("moved");
    fs::rename(&source, &moved)?;
    fs::write(moved.join("unmatched.md"), "# Different")?;
    let report = store.relocate_source(&store.settings()?.sources[0].id, moved.clone())?;
    assert!(report.issues.iter().any(|s| s.contains("content differs")));
    assert_eq!(
        store.get(&original.id)?.spec_path,
        fs::canonicalize(moved.join("one.md"))?
    );
    assert_eq!(store.get(&unmatched.id)?.spec_path, unmatched.spec_path);
    let unrelated = root.join("other");
    fs::create_dir(&unrelated)?;
    fs::write(unrelated.join("one.md"), "# One")?;
    let mut settings = store.settings()?;
    settings.sources.clear();
    settings.sources.push(SpecSource::new(unrelated)?);
    store.save_settings(&settings)?;
    assert_eq!(store.scan()?.imported, 1);
    assert_eq!(
        store.get(&original.id)?.spec_path,
        fs::canonicalize(moved.join("one.md"))?
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn active_session_requires_settle_before_relink_and_relocation() -> Result<()> {
    let (root, store, source) = fixture()?;
    fs::write(source.join("one.md"), "# One")?;
    store.scan()?;
    let record = store.list()?.remove(0);
    let mut active = record.clone();
    active.launch = Some(Launch {
        status: "prompt_sent".into(),
        harness: "codex".into(),
        workspace: "w".into(),
        workspace_id: None,
        tab_id: None,
        pane_id: None,
        agent: None,
        model: "m".into(),
        effort: "high".into(),
        prompt: "p".into(),
        error: None,
    });
    store.write(&active)?;
    let destination = root.join("relinked.md");
    fs::write(&destination, "# Different")?;
    assert!(store.relink(&record.id, destination.clone()).is_err());
    assert!(
        store
            .relocate_source(&store.settings()?.sources[0].id, source.clone())
            .is_err()
    );
    assert!(
        store
            .update(
                &record.id,
                Change::BeginRefinement(
                    Box::new(active.launch.clone().unwrap()),
                    record.spec_path.clone()
                )
            )
            .is_err()
    );
    store.settle(&record.id)?;
    let relinked = store.relink(&record.id, destination)?;
    assert!(
        store
            .update(
                &record.id,
                Change::BeginRefinement(
                    Box::new(active.launch.clone().unwrap()),
                    record.spec_path.clone()
                )
            )
            .is_err()
    );
    assert!(
        store
            .update(
                &record.id,
                Change::Launch(
                    Box::new(active.launch.clone().unwrap()),
                    record.spec_path.clone()
                )
            )
            .is_err()
    );
    assert_eq!(
        (relinked.id, relinked.spec, relinked.jira.status),
        (record.id, "done".into(), "ready".into())
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn legacy_records_preserve_progress_and_future_versions_are_readonly() -> Result<()> {
    let (root, store, source) = fixture()?;
    let existing = store.start("Legacy", None, Some(source.join("old.md")))?;
    let mut json = serde_json::to_value(&existing)?;
    for field in [
        "schema_version",
        "ownership",
        "source_id",
        "source_relative_path",
        "content_fingerprint",
    ] {
        json.as_object_mut().unwrap().remove(field);
    }
    fs::write(store.path_for(&existing.id)?, serde_json::to_vec(&json)?)?;
    assert_eq!(store.scan()?.known, 1);
    assert_eq!(store.get(&existing.id)?.spec, "in_progress");
    assert_eq!(store.get(&existing.id)?.ownership, "user");
    let mut json = serde_json::to_value(store.get(&existing.id)?)?;
    json["schema_version"] = 99.into();
    let future = serde_json::to_vec(&json)?;
    fs::write(store.path_for(&existing.id)?, &future)?;
    assert!(
        store
            .update(
                &existing.id,
                Change::Title {
                    title: "Overwrite".into()
                }
            )
            .is_err()
    );
    assert_eq!(fs::read(store.path_for(&existing.id)?)?, future);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn creation_uses_selected_folder_and_rejects_existing_agent_target() -> Result<()> {
    let (root, store, source) = fixture()?;
    let new = store.start_untitled(None)?;
    assert_eq!(new.spec_path.parent(), Some(source.as_path()));
    assert_eq!(new.ownership, "user");
    let existing = source.join("existing.md");
    fs::write(&existing, "# Keep")?;
    assert!(
        store
            .start_untitled_with_spec(None, Some(existing.clone()))
            .is_err()
    );
    assert_eq!(fs::read_to_string(existing)?, "# Keep");
    assert!(store.settings_path().is_file());
    let mut settings = store.settings()?;
    settings.sources[0].path = root.clone();
    assert!(store.save_settings(&settings).is_err());
    fs::remove_dir_all(root)?;
    Ok(())
}
