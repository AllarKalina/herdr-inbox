use super::*;
use std::os::unix::fs::PermissionsExt;

#[test]
fn settings_persist_local_defaults_filters_and_refuse_future_version() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-settings-{}", Uuid::new_v4()));
    let source = root.join("source");
    fs::create_dir_all(source.join("nested"))?;
    let store = Store::new(root.join("app"));
    assert!(store.settings()?.sources.is_empty());
    let mut settings = store.settings()?;
    settings.sources.push(SpecSource::new(source.clone())?);
    settings.sources[0].exclude = vec!["nested/**".into()];
    settings.context_paths.push(source.clone());
    settings.preferred_client = Some("codex".into());
    settings.workspace = "my-workspace".into();
    store.save_settings(&settings)?;
    let loaded = Store::new(root.join("app")).settings()?;
    assert_eq!(loaded.workspace, "my-workspace");
    assert_eq!(loaded.preferred_client.as_deref(), Some("codex"));
    assert_eq!(loaded.sources[0].id, settings.sources[0].id);
    loaded.validate_context()?;
    fs::write(source.join("root.md"), "# Root")?;
    fs::write(source.join("nested/ignored.md"), "# Nested")?;
    assert_eq!(store.scan()?.imported, 1);
    let mut invalid = loaded.clone();
    invalid.sources[0].include = vec!["[".into()];
    assert!(store.save_settings(&invalid).is_err());
    assert_eq!(
        store.settings()?.sources[0].include,
        loaded.sources[0].include
    );
    let future = "schema_version = 99\n";
    fs::write(store.settings_path(), future)?;
    assert!(store.save_settings(&Settings::default()).is_err());
    assert_eq!(fs::read_to_string(store.settings_path())?, future);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn context_preflight_requires_readable_reference_and_missing_roots_are_not_created() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-reference-{}", Uuid::new_v4()));
    fs::create_dir(&root)?;
    let mut settings = Settings::default();
    settings.context_paths.push(root.join("missing.md"));
    assert!(settings.validate_context().is_err());
    assert!(SpecSource::new(root.join("missing source")).is_err());
    let source = root.join("source");
    fs::create_dir(&source)?;
    fs::set_permissions(&source, fs::Permissions::from_mode(0o755))?;
    settings.sources.push(SpecSource::new(source.clone())?);
    settings.context_paths.clear();
    let store = Store::new(root.join("app"));
    store.save_settings(&settings)?;
    store.start("New", None, None)?;
    assert_eq!(fs::metadata(&source)?.permissions().mode() & 0o777, 0o755);
    fs::rename(&source, root.join("moved"))?;
    assert!(store.start("Wrong", None, None).is_err());
    assert!(!source.exists());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn legacy_completed_launch_is_historical_and_legacy_managed_trash_restores() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-migration-{}", Uuid::new_v4()));
    let store = Store::new(root.clone());
    let mut record = store.start("Legacy", None, None)?;
    record.spec = "done".into();
    record.launch = Some(Launch {
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
    let mut value = serde_json::to_value(&record)?;
    for field in ["schema_version", "ownership"] {
        value.as_object_mut().unwrap().remove(field);
    }
    fs::write(store.path_for(&record.id)?, serde_json::to_vec(&value)?)?;
    let loaded = store.get(&record.id)?;
    assert!(!loaded.active_spec_session());
    assert_eq!(loaded.launch.as_ref().unwrap().status, "completed");
    assert!(store.manages_spec(&loaded));
    store.delete(&record.id)?;
    assert!(!record.spec_path.exists());
    let restored = store.restore(&record.id)?;
    assert_eq!(restored.id, record.id);
    assert!(record.spec_path.is_file());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn unreadable_sources_and_files_report_without_deleting_records() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-permissions-{}", Uuid::new_v4()));
    let source = root.join("source");
    fs::create_dir_all(&source)?;
    let spec = source.join("one.md");
    fs::write(&spec, "# One")?;
    let store = Store::new(root.join("app"));
    let mut settings = store.settings()?;
    settings.sources.push(SpecSource::new(source.clone())?);
    store.save_settings(&settings)?;
    store.scan()?;
    let id = store.list()?.remove(0).id;
    fs::set_permissions(&spec, fs::Permissions::from_mode(0o000))?;
    let file_report = store.scan()?;
    fs::set_permissions(&spec, fs::Permissions::from_mode(0o600))?;
    assert!(
        file_report
            .issues
            .iter()
            .any(|issue| issue.contains("Spec unavailable"))
    );
    fs::set_permissions(&source, fs::Permissions::from_mode(0o000))?;
    let source_report = store.scan()?;
    fs::set_permissions(&source, fs::Permissions::from_mode(0o700))?;
    assert!(
        source_report
            .issues
            .iter()
            .any(|issue| issue.contains("Cannot scan"))
    );
    assert_eq!(store.get(&id)?.id, id);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn nonrecursive_filters_leave_nested_content_untouched() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-shallow-{}", Uuid::new_v4()));
    let source = root.join("source");
    fs::create_dir_all(source.join("nested"))?;
    fs::write(source.join("one.markdown"), "# One")?;
    fs::write(source.join("nested/two.md"), "# Two")?;
    let store = Store::new(root.join("app"));
    let mut settings = store.settings()?;
    let mut spec_source = SpecSource::new(source.clone())?;
    spec_source.recursive = false;
    settings.sources.push(spec_source);
    store.save_settings(&settings)?;
    assert_eq!(store.scan()?.imported, 1);
    assert_eq!(fs::read_to_string(source.join("nested/two.md"))?, "# Two");
    fs::remove_dir_all(root)?;
    Ok(())
}
