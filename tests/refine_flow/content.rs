use super::*;

#[test]
fn configured_context_reaches_new_and_refinement_prompts_and_explicit_targets_stay_pinned() {
    let fixture = Fixture::new();
    let context = fixture.root.join("context ' notes");
    let specs = fixture.root.join("custom specs");
    fs::create_dir(&context).unwrap();
    fs::create_dir(&specs).unwrap();
    fixture.success(&["settings", "add-source", specs.to_str().unwrap()]);
    fixture.success(&["settings", "add-context", context.to_str().unwrap()]);
    let spec = specs.join("chosen filename.md");
    fixture.success(&[
        "launch",
        "--profile",
        "codex",
        "--spec",
        spec.to_str().unwrap(),
    ]);
    let listed: Value = serde_json::from_slice(&fixture.run(&["list", "--json"]).stdout).unwrap();
    let id = listed[0]["id"].as_str().unwrap();
    let quoted = format!("'{}'", context.to_str().unwrap().replace('\'', "'\\''"));
    let initial = listed[0]["launch"]["prompt"].as_str().unwrap();
    assert!(initial.contains(&quoted));
    let spec = fs::canonicalize(&specs).unwrap().join("chosen filename.md");
    assert!(initial.contains(spec.to_str().unwrap()));
    assert!(fixture.calls().contains(&format!(
        "--add-dir {}",
        fs::canonicalize(&specs).unwrap().display()
    )));
    fixture.success(&["settings", "remove-context", context.to_str().unwrap()]);
    assert_eq!(fixture.record(id)["launch"]["prompt"], initial);
    fs::write(&spec, "# Chosen spec\n\nComplete.\n").unwrap();
    fixture.success(&["finish", id, "--title", "Chosen spec"]);
    fixture.success(&["settings", "add-context", context.to_str().unwrap()]);
    fixture.success(&["refine", id, "--profile", "codex"]);
    assert!(
        fixture.record(id)["launch"]["prompt"]
            .as_str()
            .unwrap()
            .contains(&quoted)
    );
    assert_eq!(fixture.record(id)["spec_path"], spec.to_str().unwrap());
}

#[test]
fn existing_new_destination_fails_without_item_or_tab_creation() {
    let fixture = Fixture::new();
    let spec = fixture.root.join("chosen/existing.md");
    fs::write(&spec, "# Existing\n").unwrap();
    let output = fixture.run(&[
        "launch",
        "--profile",
        "codex",
        "--spec",
        spec.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
    assert!(!fixture.calls().contains("tab create"));
    let list: Value = serde_json::from_slice(&fixture.run(&["list", "--json"]).stdout).unwrap();
    assert!(list.as_array().unwrap().is_empty());
    assert_eq!(fs::read_to_string(&spec).unwrap(), "# Existing\n");
}

#[test]
fn second_refinement_cannot_hide_an_unresolved_session_or_enable_rebinding() {
    let fixture = Fixture::new();
    let original = fixture.completed();
    let id = original["id"].as_str().unwrap();
    fixture.success(&["refine", id, "--profile", "codex"]);
    let active = fixture.record(id);
    let output = fixture
        .command(&["refine", id, "--profile", "codex"])
        .env("HERDR_FAKE_FAIL_AGENT", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Finish or settle"));
    assert_eq!(fixture.record(id), active);
    let replacement = fs::canonicalize(fixture.root.join("chosen"))
        .unwrap()
        .join("replacement.md");
    fs::write(&replacement, "# Replacement\n").unwrap();
    assert!(
        !fixture
            .run(&["relink", id, replacement.to_str().unwrap()])
            .status
            .success()
    );
    fixture.success(&["settle", id]);
    fixture.success(&["relink", id, replacement.to_str().unwrap()]);
}

#[test]
fn concurrent_relink_during_workspace_lookup_cannot_retarget_a_preflighted_prompt() {
    let fixture = Fixture::new();
    let original = fixture.completed();
    let id = original["id"].as_str().unwrap();
    let replacement = fs::canonicalize(fixture.root.join("chosen"))
        .unwrap()
        .join("replacement.md");
    fs::write(&replacement, "# Replacement\n").unwrap();
    let script = fs::read_to_string(&fixture.herdr).unwrap().replace(
        "case \"$1 $2\" in",
        "if [ \"$1 $2\" = \"workspace list\" ] && [ -n \"$INBOX_RACE_ID\" ]; then\n  \"$INBOX_RACE_EXE\" relink \"$INBOX_RACE_ID\" \"$INBOX_RACE_SPEC\" >/dev/null || exit 1\nfi\ncase \"$1 $2\" in",
    );
    fs::write(&fixture.herdr, script).unwrap();
    let output = fixture
        .command(&["refine", id, "--profile", "codex"])
        .env("INBOX_RACE_ID", id)
        .env("INBOX_RACE_EXE", env!("CARGO_BIN_EXE_herdr-inbox"))
        .env("INBOX_RACE_SPEC", &replacement)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Spec location changed"));
    assert!(!fixture.calls().contains("tab create"));
    let record = fixture.record(id);
    assert_eq!(record["spec_path"], replacement.to_str().unwrap());
    assert_eq!(record["launch"], original["launch"]);
}
