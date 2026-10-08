use serde_json::Value;
use std::fs;
use std::process::Command;
use uuid::Uuid;

#[test]
fn cli_archive_requires_full_matching_confirmation_and_keeps_source_file() {
    let root = std::env::temp_dir().join(format!("herdr-inbox-delete-{}", Uuid::new_v4()));
    let exe = env!("CARGO_BIN_EXE_herdr-inbox");
    let run = |args: &[&str]| {
        Command::new(exe)
            .args(args)
            .env("HERDR_INBOX_HOME", &root)
            .output()
            .unwrap()
    };
    let source = root.join("chosen");
    fs::create_dir_all(&source).unwrap();
    assert!(
        run(&["settings", "add-source", source.to_str().unwrap()])
            .status
            .success()
    );
    let started = run(&["start", "Archive test"]);
    assert!(started.status.success());
    let records: Value = serde_json::from_slice(&run(&["list", "--json"]).stdout).unwrap();
    let id = records[0]["id"].as_str().unwrap();
    let spec = records[0]["spec_path"].as_str().unwrap();

    assert!(!run(&["archive", id]).status.success());
    assert!(
        !run(&["archive", id, "--confirm", &id[..8]])
            .status
            .success()
    );
    assert!(!run(&["delete", id, "--confirm", id]).status.success());
    assert!(run(&["show", id]).status.success());
    assert!(std::path::Path::new(spec).is_file());

    assert!(run(&["archive", id, "--confirm", id]).status.success());
    assert!(!run(&["show", id]).status.success());
    assert!(std::path::Path::new(spec).is_file());
    assert!(
        root.join("trash/items")
            .join(format!("{id}.json"))
            .is_file()
    );
    fs::remove_dir_all(root).unwrap();
}
