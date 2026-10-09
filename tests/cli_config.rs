//! End-to-end CLI regression for configuration: selecting folders, filters, context
//! references, defaults, and what a scan reports.

mod common;

use common::Cli;
use std::fs;

#[test]
fn adding_a_folder_imports_its_specs_at_once_and_leaves_the_folder_untouched() {
    let cli = Cli::unconfigured("add-source");
    let spec = cli.spec("nested/plan.md", "# Imported plan\n\nBody.\n");
    cli.spec("notes.txt", "Not a spec");
    let added = cli.ok(&["settings", "add-source", cli.source.to_str().unwrap()]);
    assert!(added.contains("Source: "), "{added}");
    assert!(added.contains("1 imported"), "{added}");

    let records = cli.json(&["list", "--json"]);
    let record = &records[0];
    assert_eq!(records.as_array().unwrap().len(), 1);
    assert_eq!(record["title"], "Imported plan");
    assert_eq!(record["spec"], "done");
    assert_eq!(record["jira"]["status"], "ready");
    assert_eq!(record["source_relative_path"], "nested/plan.md");
    assert_eq!(
        record["spec_path"],
        fs::canonicalize(&spec).unwrap().to_str().unwrap()
    );
    assert_eq!(
        fs::read_to_string(&spec).unwrap(),
        "# Imported plan\n\nBody.\n"
    );

    let settings = cli.json(&["settings", "show", "--json"]);
    assert_eq!(settings["schema_version"], 1);
    assert_eq!(settings["jira"], true);
    assert_eq!(settings["workspace"], "ai-boiler-room");
    let source = &settings["sources"][0];
    assert_eq!(source["id"], record["source_id"]);
    assert_eq!(source["recursive"], true);
    assert_eq!(
        source["include"],
        serde_json::json!(["**/*.md", "**/*.markdown"])
    );
}

#[test]
fn filters_and_flat_folders_decide_which_files_are_specs() {
    let cli = Cli::unconfigured("filters");
    cli.spec("selected-notes.md", "# Selected root\n");
    cli.spec("skip-notes.md", "# Excluded root\n");
    cli.spec("other.md", "# Filtered out\n");
    cli.spec("arbitrary/selected-nested.md", "# Nested\n");
    let source = cli.source.to_str().unwrap();
    cli.ok(&["settings", "add-source", source, "--flat"]);
    let id = cli.json(&["settings", "show", "--json"])["sources"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(cli.json(&["list", "--json"]).as_array().unwrap().len(), 3);

    // Removing the folder empties the Inbox view of it without touching files.
    assert!(
        cli.err(&["settings", "remove-source", "no-such-id"])
            .contains("not found")
    );
    cli.ok(&["settings", "remove-source", &id]);
    assert!(cli.source.join("other.md").is_file());

    let filters = ["--flat", "--include", "*-notes.md", "--exclude", "skip*"];
    let mut args = vec!["settings", "add-source", source];
    args.extend(filters);
    cli.ok(&args);
    let settings = cli.json(&["settings", "show", "--json"]);
    assert_eq!(
        settings["sources"][0]["include"],
        serde_json::json!(["*-notes.md"])
    );
    assert_eq!(settings["sources"][0]["recursive"], false);
    let scan = cli.json(&["scan", "--json"]);
    assert!(scan["issues"].as_array().unwrap().is_empty(), "{scan}");
}

#[test]
fn defaults_and_context_references_are_validated_and_stored() {
    let cli = Cli::new("defaults");
    assert!(
        cli.err(&["settings", "defaults", "--profile", "gpt"])
            .contains("Unknown profile")
    );
    cli.ok(&[
        "settings",
        "defaults",
        "--profile",
        "codex",
        "--workspace",
        "work",
    ]);
    let settings = cli.json(&["settings", "show", "--json"]);
    assert_eq!(settings["preferred_client"], "codex");
    assert_eq!(settings["workspace"], "work");

    let context = cli.root.join("local context.txt");
    assert!(
        !cli.run(&["settings", "add-context", context.to_str().unwrap()])
            .status
            .success()
    );
    fs::write(&context, "Read me when relevant.\n").unwrap();
    cli.ok(&["settings", "add-context", context.to_str().unwrap()]);
    cli.ok(&["settings", "add-context", context.to_str().unwrap()]);
    let listed = cli.json(&["settings", "show", "--json"]);
    assert_eq!(listed["context_paths"].as_array().unwrap().len(), 1);

    // A reference that has gone missing stops a session before anything is created.
    fs::remove_file(&context).unwrap();
    let blocked = |args: &[&str]| {
        let error = cli.err(args);
        assert!(error.to_lowercase().contains("context"), "{error}");
        assert!(error.contains(context.to_str().unwrap()), "{error}");
    };
    blocked(&["launch", "--repo", cli.root.to_str().unwrap()]);
    assert_eq!(cli.json(&["list", "--json"]), serde_json::json!([]));
    assert_eq!(fs::read_dir(&cli.source).unwrap().count(), 0);
    cli.spec("completed.md", "# Existing spec\n");
    cli.ok(&["scan"]);
    let before = cli.json(&["list", "--json"]);
    blocked(&["refine", before[0]["id"].as_str().unwrap()]);
    assert_eq!(cli.json(&["list", "--json"]), before);

    cli.ok(&["settings", "remove-context", context.to_str().unwrap()]);
    assert!(
        cli.err(&["settings", "remove-context", context.to_str().unwrap()])
            .contains("not found")
    );
}

#[test]
fn a_moved_folder_is_reported_by_scan_and_followed_by_relocate() {
    let cli = Cli::new("relocate");
    cli.spec("one.md", "# One\n");
    cli.ok(&["scan"]);
    let before = cli.json(&["list", "--json"]);
    let id = before[0]["id"].as_str().unwrap();
    cli.ok(&["jira", id, "MOVE-1"]);
    let source = cli.json(&["settings", "show", "--json"])["sources"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let moved = cli.root.join("moved specs");
    fs::rename(&cli.source, &moved).unwrap();
    // The folder being gone is an issue, not a reason to forget its items.
    let scan = cli.json(&["scan", "--json"]);
    assert_eq!(scan["dropped"], 0);
    assert!(
        scan["issues"][0]
            .as_str()
            .unwrap()
            .contains("Source unavailable"),
        "{scan}"
    );
    assert_eq!(cli.json(&["show", id, "--json"])["jira"]["key"], "MOVE-1");

    let target = moved.to_str().unwrap();
    assert!(
        cli.err(&["settings", "relocate-source", &source, target])
            .contains("--confirm")
    );
    let relocated = cli.ok(&["settings", "relocate-source", &source, target, "--confirm"]);
    assert!(relocated.contains("0 imported, 1 known"), "{relocated}");
    let record = cli.json(&["show", id, "--json"]);
    assert_eq!(record["jira"]["key"], "MOVE-1");
    let path = fs::canonicalize(moved.join("MOVE-1-one.md")).unwrap();
    assert_eq!(record["spec_path"], path.to_str().unwrap());
}
