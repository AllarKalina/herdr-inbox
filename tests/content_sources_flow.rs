use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use uuid::Uuid;

struct Fixture {
    root: PathBuf,
    data: PathBuf,
    source: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("herdr-inbox-content-{}", Uuid::new_v4()));
        let source = root.join("my arbitrary knowledge folder");
        fs::create_dir_all(&source).unwrap();
        Self {
            data: root.join("application support"),
            root,
            source,
        }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_herdr-inbox"))
            .args(args)
            .env("HERDR_INBOX_HOME", &self.data)
            .env("HERDR_ENV", "0")
            .env_remove("HERDR_PLUGIN_ID")
            .output()
            .unwrap()
    }

    fn ok(&self, args: &[&str]) -> Output {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn json(&self, args: &[&str]) -> Value {
        serde_json::from_slice(&self.ok(args).stdout).unwrap()
    }

    fn list(&self) -> Value {
        self.json(&["list", "--json"])
    }

    fn configure(&self) {
        self.ok(&["settings", "add-source", self.source.to_str().unwrap()]);
    }

    fn spec(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.source.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }

    fn enrich(&self, id: &str) {
        self.ok(&[
            "jira",
            id,
            "TEST-42",
            "--url",
            "https://jira.example/TEST-42",
        ]);
        self.ok(&[
            "implement",
            id,
            "--agent",
            "codex",
            "--branch",
            "feature/42",
        ]);
        self.ok(&["pr", id, "https://github.example/team/project/pull/42"]);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn settings_import_arbitrary_nested_markdown_without_touching_content() {
    let fixture = Fixture::new();
    let spec = fixture.spec(
        "loose notes/something unexpected.markdown",
        "intro\n# My spec\n\nDetails.\n",
    );
    fixture.spec("plain filename.md", "No heading here.\n");
    fixture.spec("context.txt", "Not an inbox spec.\n");
    fixture.configure();

    let records = fixture.list();
    assert_eq!(records.as_array().unwrap().len(), 2);
    let imported = records
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["title"] == "My spec")
        .unwrap();
    assert_eq!(
        imported["spec_path"],
        spec.canonicalize().unwrap().to_str().unwrap()
    );
    assert_eq!(imported["ownership"], "user");
    assert_eq!(imported["spec"], "done");
    assert_eq!(imported["jira"]["status"], "ready");
    assert_eq!(imported["jira"]["key"], Value::Null);
    assert_eq!(imported["implementation"]["status"], "waiting");
    assert_eq!(imported["pr"]["status"], "waiting");
    assert_eq!(imported["launch"], Value::Null);
    assert_eq!(imported["previous_launches"], serde_json::json!([]));
    assert!(
        records
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["title"] == "plain filename")
    );
    let id = imported["id"].as_str().unwrap();
    assert!(!fixture.run(&["implement", id]).status.success());
    assert!(
        !fixture
            .run(&["pr", id, "https://github.example/pull/1"])
            .status
            .success()
    );
    assert_eq!(
        fs::read_to_string(spec).unwrap(),
        "intro\n# My spec\n\nDetails.\n"
    );
    assert_eq!(fs::read_dir(&fixture.source).unwrap().count(), 3);
    assert_eq!(
        fs::read_dir(fixture.source.join("loose notes"))
            .unwrap()
            .count(),
        1
    );
    assert!(fixture.data.join("settings.toml").is_file());
    assert!(!fixture.data.join("specs").exists());
}

#[test]
fn rescanning_preserves_enriched_records_and_imports_only_new_files() {
    let fixture = Fixture::new();
    let path = fixture.spec("a.md", "# Initial title\n\nDetails.\n");
    fixture.configure();
    let records = fixture.list();
    let id = records[0]["id"].as_str().unwrap();
    fixture.enrich(id);
    fixture.ok(&["title", id, "Curated inbox title"]);
    let enriched = fixture.json(&["show", id, "--json"]);
    fs::write(path, "# A changed source heading\n\nEdited externally.\n").unwrap();

    let report = fixture.json(&["scan", "--json"]);
    assert_eq!(report["imported"], 0);
    assert_eq!(fixture.json(&["show", id, "--json"]), enriched);
    fixture.spec("entirely different path/new.md", "# New discovery\n");
    let report = fixture.json(&["scan", "--json"]);
    assert_eq!(report["imported"], 1);
    assert_eq!(fixture.list().as_array().unwrap().len(), 2);
    assert_eq!(fixture.json(&["show", id, "--json"]), enriched);
    assert_eq!(fixture.json(&["scan", "--json"])["imported"], 0);
}

#[test]
fn deleting_imported_metadata_suppresses_discovery_until_restore() {
    let fixture = Fixture::new();
    let path = fixture.spec("retained source.md", "# Keep this document\n");
    fixture.configure();
    let records = fixture.list();
    let id = records[0]["id"].as_str().unwrap();
    fixture.enrich(id);
    let enriched = fixture.json(&["show", id, "--json"]);

    fixture.ok(&["delete", id, "--confirm", id]);
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Keep this document\n");
    assert!(
        fixture
            .data
            .join("trash/items")
            .join(format!("{id}.json"))
            .is_file()
    );
    assert_eq!(fixture.list(), serde_json::json!([]));
    let scan = fixture.json(&["scan", "--json"]);
    assert_eq!(scan["imported"], 0);
    assert_eq!(scan["suppressed"], 1);
    assert_eq!(fixture.list(), serde_json::json!([]));

    fixture.ok(&["restore", id]);
    assert_eq!(fixture.json(&["show", id, "--json"]), enriched);
    assert_eq!(fixture.json(&["scan", "--json"])["imported"], 0);
    assert_eq!(fixture.list().as_array().unwrap().len(), 1);
}

#[test]
fn two_machine_roots_have_independent_settings_uuids_and_workflow() {
    let work = Fixture::new();
    let personal = Fixture::new();
    work.spec("same filename.md", "# Work spec\n");
    personal.spec("same filename.md", "# Personal spec\n");
    work.configure();
    personal.configure();
    let work_record = work.list()[0].clone();
    let personal_record = personal.list()[0].clone();
    assert_ne!(work_record["id"], personal_record["id"]);
    work.enrich(work_record["id"].as_str().unwrap());
    assert_eq!(personal.list()[0], personal_record);
    let work_settings = work.json(&["settings", "show", "--json"]);
    let personal_settings = personal.json(&["settings", "show", "--json"]);
    assert_eq!(
        Path::new(work_settings["sources"][0]["path"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        work.source.canonicalize().unwrap()
    );
    assert_eq!(
        Path::new(personal_settings["sources"][0]["path"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        personal.source.canonicalize().unwrap()
    );
    assert_ne!(
        work_settings["sources"][0]["id"],
        personal_settings["sources"][0]["id"]
    );
    assert!(
        !fs::read_to_string(work.data.join("settings.toml"))
            .unwrap()
            .contains(personal.source.to_str().unwrap())
    );
}

#[test]
fn unavailable_sources_report_issues_without_deleting_metadata() {
    let fixture = Fixture::new();
    fixture.spec("a.md", "# Available initially\n");
    fixture.configure();
    let before = fixture.list();
    fs::rename(&fixture.source, fixture.root.join("moved independently")).unwrap();
    let report = fixture.json(&["scan", "--json"]);
    assert_eq!(report["imported"], 0);
    let issues = report["issues"].as_array().unwrap();
    assert!(!issues.is_empty());
    assert!(issues.iter().any(|issue| {
        issue
            .as_str()
            .unwrap()
            .contains(fixture.source.to_str().unwrap())
    }));
    assert_eq!(fixture.list(), before);
}

#[test]
fn missing_context_blocks_new_and_refinement_preflight_without_mutations() {
    let fixture = Fixture::new();
    fixture.configure();
    let context = fixture.root.join("local context.txt");
    fs::write(&context, "Read me when relevant.\n").unwrap();
    fixture.ok(&["settings", "add-context", context.to_str().unwrap()]);
    fs::remove_file(&context).unwrap();

    let assert_context_error = |output: Output| {
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.to_lowercase().contains("context"), "{error}");
        assert!(error.contains(context.to_str().unwrap()), "{error}");
    };
    assert_context_error(fixture.run(&["launch", "--repo", fixture.root.to_str().unwrap()]));
    assert_eq!(fixture.list(), serde_json::json!([]));
    assert!(!fixture.data.join("specs").exists());
    assert_eq!(fs::read_dir(&fixture.source).unwrap().count(), 0);

    fixture.spec("completed.md", "# Existing spec\n");
    fixture.ok(&["scan"]);
    let before = fixture.list();
    let id = before[0]["id"].as_str().unwrap();
    assert_context_error(fixture.run(&["refine", id, "--repo", fixture.root.to_str().unwrap()]));
    assert_eq!(fixture.list(), before);
}

#[test]
fn explicit_filters_and_flat_sources_do_not_impose_directory_layout() {
    let fixture = Fixture::new();
    fixture.spec("selected-notes.md", "# Selected root\n");
    fixture.spec("skip-notes.md", "# Excluded root\n");
    fixture.spec("other.md", "# Filtered out\n");
    fixture.spec("arbitrary/selected-nested.md", "# Nested\n");
    fixture.ok(&[
        "settings",
        "add-source",
        fixture.source.to_str().unwrap(),
        "--flat",
        "--include",
        "*-notes.md",
        "--exclude",
        "skip*",
    ]);
    let records = fixture.list();
    assert_eq!(records.as_array().unwrap().len(), 1);
    assert_eq!(records[0]["title"], "Selected root");
    assert!(Path::new(records[0]["spec_path"].as_str().unwrap()).is_file());
}
