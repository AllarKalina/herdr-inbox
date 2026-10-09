use super::*;

#[test]
fn cached_archive_provenance_cannot_suppress_an_unrelated_selected_file() -> Result<()> {
    let (root, store, source) = fixture()?;
    let selected = source.join("selected.md");
    let mut record = store.start("Selected", None, Some(selected.clone()))?;
    let outside = root.join("outside.md");
    fs::write(&outside, "# Outside\n")?;
    record.spec_path = outside;
    // Source metadata is descriptive; it cannot impersonate a different physical file.
    store.write(&record)?;
    store.archive(&record.id)?;
    assert_eq!(store.scan()?.imported, 1);
    let imported = store.list()?;
    assert_eq!(imported.len(), 1);
    assert_ne!(imported[0].id, record.id);
    assert_eq!(imported[0].spec_path, fs::canonicalize(selected)?);
    fs::remove_dir_all(root)?;
    Ok(())
}
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
    assert_eq!(record.implementation_stage(true), "locked");
    assert_eq!(record.pr_stage(true), "locked");
    assert_eq!(
        record.source_relative_path,
        PathBuf::from("nested/anything.markdown")
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
    assert!(
        enriched
            .spec_path
            .ends_with("nested/TEST-1-my-spec.markdown")
    );
    fs::write(&enriched.spec_path, "# Changed content")?;
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
    store.archive(&record.id)?;
    assert!(enriched.spec_path.is_file());
    assert_eq!(store.scan()?.archived, 1);
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
        status: LaunchStatus::PromptSent,
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
    let destination = source.join("relinked.md");
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
        (record.id, SpecStatus::Done, JiraStatus::Ready)
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn creation_uses_selected_folder_and_rejects_existing_agent_target() -> Result<()> {
    let (root, store, source) = fixture()?;
    let new = store.start_untitled(None, None)?;
    assert_eq!(
        new.spec_path.parent(),
        Some(fs::canonicalize(&source)?.as_path())
    );
    let existing = source.join("existing.md");
    fs::write(&existing, "# Keep")?;
    assert!(store.start_untitled(None, Some(existing.clone())).is_err());
    assert_eq!(fs::read_to_string(existing)?, "# Keep");
    assert!(store.settings_path().is_file());
    let mut settings = store.settings()?;
    settings.sources[0].path = root.clone();
    assert!(store.save_settings(&settings).is_err());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn creation_requires_sources_and_rejects_outside_destinations() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-no-fallback-{}", Uuid::new_v4()));
    let empty = Store::new(root.join("app"));
    assert!(
        empty
            .start("Wrong", None, None)
            .unwrap_err()
            .to_string()
            .contains("settings")
    );
    assert!(empty.start_untitled(None, None).is_err());
    assert!(!empty.path().join("specs").exists());
    let source = root.join("selected");
    fs::create_dir_all(&source)?;
    let mut settings = Settings::default();
    settings.sources.push(SpecSource::new(source.clone())?);
    empty.save_settings(&settings)?;
    let outside = root.join("outside/new.md");
    assert!(empty.start("Wrong", None, Some(outside.clone())).is_err());
    assert!(!outside.exists());
    assert!(!root.join("outside").exists());
    fs::create_dir(root.join("external"))?;
    symlink(root.join("external"), source.join("escape"))?;
    assert!(
        empty
            .start("Wrong", None, Some(source.join("escape/new.md")))
            .is_err()
    );
    assert!(
        empty
            .start("Wrong", None, Some(source.join("../wrong.md")))
            .is_err()
    );
    assert!(
        empty
            .start("Wrong", None, Some(source.join("wrong.txt")))
            .is_err()
    );
    empty.start("Current", None, None)?;
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn discovery_never_imports_files_or_directories_outside_selected_folder() -> Result<()> {
    let (root, store, source) = fixture()?;
    let outside = root.join("external");
    fs::create_dir(&outside)?;
    fs::write(outside.join("secret.md"), "# Outside")?;
    fs::write(source.join("inside.md"), "# Inside")?;
    symlink(outside.join("secret.md"), source.join("alias.md"))?;
    symlink(&outside, source.join("linked-folder"))?;
    let report = store.scan()?;
    assert_eq!(report.imported, 1);
    assert_eq!(store.list()?.remove(0).title, "Inside");
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.contains("outside selected folder"))
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn creation_and_discovery_share_canonical_scope_and_ancestor_exclusions() -> Result<()> {
    let (root, store, source) = fixture()?;
    let nested = source.join("nested");
    fs::create_dir(nested.join("ignored"))?;
    let mut settings = store.settings()?;
    let mut child = SpecSource::new(nested.clone())?;
    child.exclude = vec!["ignored".into()];
    let child_id = child.id.clone();
    settings.sources.push(child);
    settings.sources[0].exclude = vec!["nested/ignored".into()];
    store.save_settings(&settings)?;
    let created = store.start("Created", None, Some(nested.join("new.md")))?;
    assert_eq!(created.source_id, child_id);
    assert_eq!(created.source_relative_path, PathBuf::from("new.md"));
    assert!(
        store
            .start("Excluded", None, Some(nested.join("ignored/new.md")))
            .is_err()
    );
    fs::write(nested.join("scan.md"), "# Scanned")?;
    fs::write(nested.join("ignored/hidden.md"), "# Hidden")?;
    symlink(nested.join("ignored/hidden.md"), source.join("alias.md"))?;
    assert_eq!(store.scan()?.imported, 1);
    let scanned = store
        .list()?
        .into_iter()
        .find(|record| record.title == "Scanned")
        .unwrap();
    assert_eq!(scanned.source_id, child_id);
    assert_eq!(scanned.source_relative_path, PathBuf::from("scan.md"));
    assert_eq!(store.list()?.len(), 2);
    fs::remove_dir_all(root)?;
    Ok(())
}
