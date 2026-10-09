use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use uuid::Uuid;

struct Fixture {
    root: PathBuf,
    repo: PathBuf,
    data: PathBuf,
    log: PathBuf,
    herdr: PathBuf,
    path: String,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("herdr-inbox-refine-{}", Uuid::new_v4()));
        let bin = root.join("bin");
        let repo = root.join("repo with ' quote");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(&repo).unwrap();
        let herdr = bin.join("herdr-fake");
        fs::write(&herdr, r##"#!/bin/sh
printf '%s\n' "$*" >> "$HERDR_FAKE_LOG"
if [ "$1 $2" = "agent start" ] && [ "$HERDR_FAKE_FAIL_AGENT" = "1" ]; then
  printf '%s\n' 'refine-test-agent-failure' >&2
  exit 1
fi
if [ "$1 $2" = "workspace focus" ] && [ "$HERDR_FAKE_FAIL_FOCUS" = "1" ]; then
  printf '%s\n' 'refine-test-focus-failure' >&2
  exit 1
fi
case "$1 $2" in
  "workspace list") printf '%s\n' '{"result":{"workspaces":[{"label":"ai-boiler-room","workspace_id":"w2"}]}}' ;;
  "tab create")
    count=0
    if [ -f "$HERDR_FAKE_LOG.count" ]; then count=$(cat "$HERDR_FAKE_LOG.count"); fi
    count=$((count + 1))
    printf '%s\n' "$count" > "$HERDR_FAKE_LOG.count"
    printf '{"result":{"tab":{"tab_id":"w2:t%s"},"root_pane":{"pane_id":"w2:p%s"}}}\n' "$count" "$count"
    ;;
  *) printf '%s\n' '{"result":{"type":"ok"}}' ;;
esac
"##).unwrap();
        for executable in [&herdr, &bin.join("claude"), &bin.join("codex")] {
            if executable != &herdr {
                fs::write(executable, "#!/bin/sh\nexit 0\n").unwrap();
            }
            fs::set_permissions(executable, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let fixture = Self {
            repo,
            data: root.join("local inbox's data"),
            log: root.join("herdr.log"),
            herdr,
            path: format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
            root,
        };
        let source = fixture.root.join("chosen");
        fs::create_dir_all(&source).unwrap();
        fixture.success(&["settings", "add-source", source.to_str().unwrap()]);
        fixture
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_herdr-inbox"));
        command
            .args(args)
            .env("HERDR_ENV", "1")
            .env("HERDR_BIN_PATH", &self.herdr)
            .env("HERDR_FAKE_LOG", &self.log)
            .env("HERDR_INBOX_HOME", &self.data)
            .env("PATH", &self.path);
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }

    fn success(&self, args: &[&str]) {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn record(&self, id: &str) -> Value {
        let output = self.run(&["show", id, "--json"]);
        assert!(output.status.success());
        serde_json::from_slice(&output.stdout).unwrap()
    }

    fn completed(&self) -> Value {
        self.success(&[
            "start",
            "Payment retries",
            "--repo",
            self.repo.to_str().unwrap(),
        ]);
        let listed: Value = serde_json::from_slice(&self.run(&["list", "--json"]).stdout).unwrap();
        let id = listed[0]["id"].as_str().unwrap();
        let spec = listed[0]["spec_path"].as_str().unwrap();
        fs::write(
            spec,
            "# Payment retries\n\nValidate retries against current source.\n",
        )
        .unwrap();
        self.success(&["finish", id]);
        self.success(&[
            "jira",
            id,
            "PAY-123",
            "--url",
            "https://jira.example/PAY-123",
        ]);
        self.success(&[
            "implement",
            id,
            "--agent",
            "implementor",
            "--branch",
            "feature/payment-retries",
        ]);
        self.success(&["pr", id, "https://github.example/org/repo/pull/42"]);
        self.record(id)
    }

    fn calls(&self) -> String {
        fs::read_to_string(&self.log).unwrap_or_default()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn refinement_uses_the_same_item_and_spec_with_both_client_profiles() {
    for (profile, model, skill) in [
        ("opus", "claude-opus-5-5", "/grill-me"),
        ("codex", "gpt-6.1-sol", "$grill-me"),
    ] {
        let fixture = Fixture::new();
        let marker = fixture.root.join("title-must-not-execute");
        let original = fixture.completed();
        let title = format!("Payment retries ' $(touch {})", marker.display());
        fixture.success(&["title", original["id"].as_str().unwrap(), &title]);
        let before = fixture.record(original["id"].as_str().unwrap());
        let id = before["id"].as_str().unwrap();
        let spec = before["spec_path"].as_str().unwrap();
        let contents = fs::read(spec).unwrap();
        fixture.success(&["refine", id, "--profile", profile]);
        let record = fixture.record(id);
        assert_eq!(record["id"], before["id"]);
        assert_eq!(record["title"], before["title"]);
        assert_eq!(record["spec_path"], before["spec_path"]);
        assert_eq!(fs::read(spec).unwrap(), contents);
        assert_eq!(record["spec"], "in_progress");
        assert_eq!(record["launch"]["status"], "prompt_sent");
        assert_eq!(record["launch"]["harness"], profile);
        assert_eq!(record["launch"]["model"], model);
        assert_eq!(record["launch"]["effort"], "high");
        assert_eq!(record["launch"]["workspace"], "ai-boiler-room");
        let prompt = record["launch"]["prompt"].as_str().unwrap();
        assert!(prompt.contains(skill));
        let quoted_spec = format!("'{}'", spec.replace('\'', "'\\''"));
        assert!(
            prompt.contains(&quoted_spec),
            "existing spec path must be shell-quoted"
        );
        let finish = format!("'{}' finish '{id}'", env!("CARGO_BIN_EXE_herdr-inbox"));
        assert!(prompt.contains(&finish));
        assert!(
            prompt.contains(&format!("{finish}.")),
            "default finish command should retain the title"
        );
        let quoted_repo = format!(
            "'{}'",
            fixture.repo.to_str().unwrap().replace('\'', "'\\''")
        );
        let quoted_inbox = format!(
            "'{}'",
            fixture.data.to_str().unwrap().replace('\'', "'\\''")
        );
        assert!(prompt.contains(&quoted_repo));
        assert!(prompt.contains(&format!("HERDR_INBOX_HOME={quoted_inbox} {finish}")));
        assert!(prompt.contains("current code and verified documentation as the source of truth"));
        let inspect = prompt.find("First read the existing spec in full").unwrap();
        let validate = prompt.find("Inspect the relevant codebases").unwrap();
        let interview = prompt
            .find("Then ask what the user wants changed or challenged")
            .unwrap();
        assert!(inspect < validate && validate < interview);
        assert!(prompt.contains("Wait for the user's answers before revising the spec"));
        assert!(prompt.contains("Do not implement code changes"));
        let calls = fixture.calls();
        assert!(calls.contains("tab create --workspace w2"));
        assert!(calls.contains(&format!("--cwd {}", fixture.repo.display())));
        assert!(calls.contains("workspace focus w2"));
        assert!(calls.contains(if profile == "codex" {
            "--kind codex"
        } else {
            "--kind claude"
        }));
        if profile == "codex" {
            assert!(calls.contains(
                "-m gpt-6.1-sol -c model_reasoning_effort=\"high\" -s workspace-write --add-dir"
            ));
            assert!(!calls.contains("--permission-mode"));
        } else {
            assert!(calls.contains("--model claude-opus-5-5 --effort high --permission-mode auto"));
        }
        let listed: Value =
            serde_json::from_slice(&fixture.run(&["list", "--json"]).stdout).unwrap();
        assert_eq!(listed.as_array().unwrap().len(), 1);
        let output = Command::new("sh")
            .args(["-c", &format!("HERDR_INBOX_HOME={quoted_inbox} {finish}")])
            .env("HERDR_ENV", "1")
            .env("HERDR_BIN_PATH", &fixture.herdr)
            .env("HERDR_FAKE_LOG", &fixture.log)
            .env("PATH", &fixture.path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let completed = fixture.record(id);
        assert!(
            !marker.exists(),
            "raw title must not become shell code in the finish command"
        );
        assert_eq!(completed["spec"], "done");
        for key in ["title", "jira", "implementation", "pr"] {
            assert_eq!(
                completed[key], before[key],
                "refinement must preserve {key}"
            );
        }
    }
}

#[test]
fn repeated_refinements_use_unique_agents_and_preserve_previous_sessions() {
    let fixture = Fixture::new();
    let before = fixture.completed();
    let id = before["id"].as_str().unwrap();
    fixture.success(&["refine", id, "--profile", "codex"]);
    let first = fixture.record(id);
    fixture.success(&["finish", id]);
    fixture.success(&["refine", id, "--profile", "codex"]);
    let second = fixture.record(id);
    assert_ne!(first["launch"]["agent"], second["launch"]["agent"]);
    for record in [&first, &second] {
        let name = record["launch"]["agent"].as_str().unwrap();
        assert!(
            name.len() <= 32,
            "Herdr agent names have a 32-character limit"
        );
        assert!(
            name.bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        );
    }
    assert_ne!(first["launch"]["tab_id"], second["launch"]["tab_id"]);
    let history = second["previous_launches"].as_array().unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0]["agent"], first["launch"]["agent"]);
    assert_eq!(history[0]["prompt"], first["launch"]["prompt"]);
    assert_eq!(history[0]["status"], "completed");
    assert_eq!(
        fixture
            .calls()
            .lines()
            .filter(|line| line.starts_with("agent start "))
            .count(),
        2
    );
    fixture.success(&["finish", id]);
    assert_eq!(fixture.record(id)["pr"], before["pr"]);
}

#[test]
fn missing_or_blank_specs_fail_before_creating_a_tab_or_changing_the_record() {
    for contents in [None, Some(" \n\t")] {
        let fixture = Fixture::new();
        let before = fixture.completed();
        let id = before["id"].as_str().unwrap();
        let spec = before["spec_path"].as_str().unwrap();
        if let Some(contents) = contents {
            fs::write(spec, contents).unwrap();
        } else {
            fs::remove_file(spec).unwrap();
        }
        let output = fixture.run(&["refine", id, "--profile", "codex"]);
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(if contents.is_none() {
            "Spec file is missing"
        } else {
            "Spec file is empty"
        }));
        assert!(error.contains(spec));
        assert!(!fixture.calls().contains("tab create"));
        assert_eq!(fixture.record(id), before);
    }
}

#[test]
fn failed_agent_launch_keeps_the_completed_spec_and_linked_work_with_diagnostics() {
    let fixture = Fixture::new();
    let before = fixture.completed();
    let id = before["id"].as_str().unwrap();
    let output = fixture
        .command(&["refine", id, "--profile", "codex"])
        .env("HERDR_FAKE_FAIL_AGENT", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let record = fixture.record(id);
    for key in ["title", "spec", "spec_path", "jira", "implementation", "pr"] {
        assert_eq!(
            record[key], before[key],
            "launch failure must preserve {key}"
        );
    }
    assert_eq!(record["launch"]["status"], "failed");
    assert!(
        record["launch"]["error"]
            .as_str()
            .unwrap()
            .contains("refine-test-agent-failure")
    );
    assert!(!fixture.calls().contains("agent prompt"));
}

#[test]
fn focus_failure_after_prompt_delivery_keeps_the_live_session_and_returns_success() {
    let fixture = Fixture::new();
    let before = fixture.completed();
    let id = before["id"].as_str().unwrap();
    let output = fixture
        .command(&["refine", id, "--profile", "codex"])
        .env("HERDR_FAKE_FAIL_FOCUS", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let record = fixture.record(id);
    assert_eq!(record["spec"], "in_progress");
    assert_eq!(record["launch"]["status"], "prompt_sent");
    assert!(
        record["launch"]["error"]
            .as_str()
            .unwrap()
            .contains("refine-test-focus-failure")
    );
    assert!(
        record["launch"]["error"]
            .as_str()
            .unwrap()
            .contains("Select the new tab manually")
    );
    assert_eq!(
        fixture
            .calls()
            .lines()
            .filter(|line| line.starts_with("agent start "))
            .count(),
        1
    );
    assert_eq!(
        fixture
            .calls()
            .lines()
            .filter(|line| line.starts_with("agent prompt "))
            .count(),
        1
    );
    fixture.success(&["finish", id]);
    assert_eq!(fixture.record(id)["pr"], before["pr"]);
}

#[test]
fn refinement_accepts_launch_overrides_without_changing_item_identity() {
    let fixture = Fixture::new();
    let before = fixture.completed();
    let id = before["id"].as_str().unwrap();
    let repo = fixture.root.join("alternate repo");
    fs::create_dir(&repo).unwrap();
    fixture.success(&[
        "refine",
        id,
        "--profile",
        "opus",
        "--repo",
        repo.to_str().unwrap(),
        "--workspace",
        "ai-boiler-room",
        "--model",
        "test-opus-model",
        "--effort",
        "medium",
        "--topic",
        "Review retry deadlines",
        "--ask-permissions",
    ]);
    let record = fixture.record(id);
    assert_eq!(record["spec_path"], before["spec_path"]);
    assert_eq!(record["title"], before["title"]);
    assert_eq!(record["launch"]["model"], "test-opus-model");
    assert_eq!(record["launch"]["effort"], "medium");
    assert!(
        record["launch"]["prompt"]
            .as_str()
            .unwrap()
            .contains("Review retry deadlines")
    );
    let calls = fixture.calls();
    assert!(calls.contains(&format!("--cwd {}", repo.display())));
    assert!(calls.contains("--model test-opus-model --effort medium"));
    assert!(!calls.contains("--permission-mode"));
}

#[test]
fn codex_refinement_grants_access_to_the_selected_spec_directory() {
    let fixture = Fixture::new();
    let drafts = fixture.root.join("external drafts ' quoted");
    fs::create_dir(&drafts).unwrap();
    fixture.success(&["settings", "add-source", drafts.to_str().unwrap()]);
    let drafts = fs::canonicalize(drafts).unwrap();
    let spec = drafts.join("payment retries.md");
    let contents = "# Existing external spec\n\nKeep this exact file.\n";
    fs::write(&spec, contents).unwrap();
    fixture.success(&[
        "start",
        "Payment retries",
        "--repo",
        fixture.repo.to_str().unwrap(),
        "--spec",
        spec.to_str().unwrap(),
    ]);
    let list: Value = serde_json::from_slice(&fixture.run(&["list", "--json"]).stdout).unwrap();
    let id = list[0]["id"].as_str().unwrap();
    fixture.success(&["finish", id]);
    fixture.success(&["refine", id, "--profile", "codex"]);
    assert!(
        fixture
            .calls()
            .contains(&format!("--add-dir {}", drafts.display()))
    );
    assert_eq!(fixture.record(id)["spec_path"], spec.to_str().unwrap());
    assert_eq!(fs::read_to_string(&spec).unwrap(), contents);
    assert_eq!(list.as_array().unwrap().len(), 1);
}

#[path = "refine_flow/content.rs"]
mod content;
