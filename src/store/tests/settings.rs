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
fn obsolete_metadata_is_rejected_without_rewriting_it() -> Result<()> {
    let root = std::env::temp_dir().join(format!("herdr-schema-{}", Uuid::new_v4()));
    let store = configured_store(&root)?;
    let record = store.start("Current", None, None)?;
    for field in ["schema_version", "previous_launches"] {
        let mut value = serde_json::to_value(&record)?;
        value.as_object_mut().unwrap().remove(field);
        let bytes = serde_json::to_vec(&value)?;
        fs::write(store.item_path(&record.id)?, &bytes)?;
        assert!(store.get(&record.id).is_err());
        assert_eq!(fs::read(store.item_path(&record.id)?)?, bytes);
    }
    for version in [0, 99] {
        let mut value = serde_json::to_value(&record)?;
        value["schema_version"] = version.into();
        let bytes = serde_json::to_vec(&value)?;
        fs::write(store.item_path(&record.id)?, &bytes)?;
        assert!(store.get(&record.id).is_err());
        assert_eq!(fs::read(store.item_path(&record.id)?)?, bytes);
    }
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
