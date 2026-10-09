use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

#[test]
fn a_new_session_chooses_where_its_spec_goes_and_reports_it_when_finishing() {
    let root = std::env::temp_dir().join(format!("herdr-inbox-launch-{}", Uuid::new_v4()));
    let bin_dir = root.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let log = root.join("herdr.log");
    let herdr = bin_dir.join("herdr-fake");
    fs::write(&herdr, r##"#!/bin/sh
printf '%s\n' "$*" >> "$HERDR_FAKE_LOG"
case "$1 $2" in
  "workspace list") printf '%s\n' '{"result":{"workspaces":[{"label":"ai-boiler-room","workspace_id":"w2"}]}}' ;;
  "tab create") printf '%s\n' '{"result":{"tab":{"tab_id":"w2:t2"},"root_pane":{"pane_id":"w2:p2"}}}' ;;
  *) printf '%s\n' '{"result":{"type":"ok"}}' ;;
esac
"##).unwrap();
    fs::write(bin_dir.join("claude"), "#!/bin/sh\nexit 0\n").unwrap();
    for path in [&herdr, &bin_dir.join("claude")] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let path = format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap());
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_herdr-inbox"));
    let run = |args: &[&str]| {
        Command::new(&exe)
            .args(args)
            .env("HERDR_ENV", "1")
            .env("HERDR_BIN_PATH", &herdr)
            .env("HERDR_FAKE_LOG", &log)
            .env("HERDR_INBOX_HOME", root.join("data"))
            .env("PATH", &path)
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
    let output = run(&["launch", "--topic", "Improve payment retries"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let list = run(&["list", "--json"]);
    let records: Value = serde_json::from_slice(&list.stdout).unwrap();
    let record = &records[0];
    assert_eq!(record["title"], "");
    assert_eq!(record["launch"]["status"], "prompt_sent");
    assert_eq!(record["launch"]["model"], "claude-opus-5-5");
    assert_eq!(record["launch"]["tab_id"], "w2:t2");
    assert!(!PathBuf::from(record["spec_path"].as_str().unwrap()).exists());
    let calls = fs::read_to_string(&log).unwrap();
    assert!(calls.contains("--model claude-opus-5-5 --effort medium --permission-mode auto"));
    assert!(calls.contains("tab create --workspace w2 --label Spec · "));
    assert!(
        !calls.contains("--cwd"),
        "sessions start in the workspace's own folder"
    );
    assert!(calls.contains("agent prompt spec_"));
    assert!(calls.contains("/grill-me Improve payment retries"));
    // The session is told the folder, not a file: it chooses the place and the name.
    let folder = fs::canonicalize(&source).unwrap();
    let id = record["id"].as_str().unwrap();
    assert!(calls.contains(&format!("one Markdown file inside '{}'", folder.display())));
    assert!(calls.contains(&format!(
        "finish '{id}' --title \"<title>\" --spec <path of the file you wrote>"
    )));

    // Finishing without saying where the spec is cannot work: nothing is at the provisional path.
    let lost = run(&["finish", id, "--title", "Payment retries"]);
    assert!(!lost.status.success());
    assert!(String::from_utf8_lossy(&lost.stderr).contains("--spec"));

    // What the session runs once it has written the spec where it belongs.
    let spec = source.join("payments/retries.md");
    fs::create_dir_all(spec.parent().unwrap()).unwrap();
    fs::write(&spec, "# Payment retries\n\nDetails.\n").unwrap();
    // Opening the Inbox meanwhile imports the new file as an item of its own.
    assert!(run(&["scan"]).status.success());
    let before: Value = serde_json::from_slice(&run(&["list", "--json"]).stdout).unwrap();
    assert_eq!(before.as_array().unwrap().len(), 2);
    let finish = run(&[
        "finish",
        id,
        "--title",
        "Payment retries",
        "--spec",
        spec.to_str().unwrap(),
    ]);
    assert!(
        finish.status.success(),
        "{}",
        String::from_utf8_lossy(&finish.stderr)
    );
    // The session's item takes the file over; the scan's placeholder is gone.
    let after: Value = serde_json::from_slice(&run(&["list", "--json"]).stdout).unwrap();
    assert_eq!(after.as_array().unwrap().len(), 1);
    assert_eq!(after[0]["id"], id);
    assert_eq!(after[0]["title"], "Payment retries");
    assert_eq!(after[0]["spec"], "done");
    assert_eq!(after[0]["source_relative_path"], "payments/retries.md");
    assert_eq!(after[0]["launch"]["status"], "completed");
    assert!(
        fs::read_to_string(&log)
            .unwrap()
            .contains("tab rename w2:t2 Payment retries")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn codex_profile_uses_sol_medium_and_codex_skill() {
    let root = std::env::temp_dir().join(format!("herdr-inbox-codex-{}", Uuid::new_v4()));
    let bin_dir = root.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let log = root.join("herdr.log");
    let herdr = bin_dir.join("herdr-fake");
    fs::write(&herdr, r##"#!/bin/sh
printf '%s\n' "$*" >> "$HERDR_FAKE_LOG"
if [ "$1 $2" = "agent start" ] && [ ! -e "$HERDR_FAKE_LOG.busy" ]; then
  touch "$HERDR_FAKE_LOG.busy"
  printf '%s\n' '{"error":{"code":"agent_pane_busy"}}' >&2
  exit 1
fi
case "$1 $2" in
  "workspace list") printf '%s\n' '{"result":{"workspaces":[{"label":"ai-boiler-room","workspace_id":"w2"}]}}' ;;
  "tab create") printf '%s\n' '{"result":{"tab":{"tab_id":"w2:t2"},"root_pane":{"pane_id":"w2:p2"}}}' ;;
  *) printf '%s\n' '{"result":{"type":"ok"}}' ;;
esac
"##).unwrap();
    let codex = bin_dir.join("codex");
    fs::write(&codex, "#!/bin/sh\nexit 0\n").unwrap();
    for path in [&herdr, &codex] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let path = format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap());
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_herdr-inbox"));
    let source = root.join("chosen");
    fs::create_dir_all(&source).unwrap();
    let configured = Command::new(&exe)
        .args(["settings", "add-source", source.to_str().unwrap()])
        .env("HERDR_INBOX_HOME", root.join("data"))
        .output()
        .unwrap();
    assert!(configured.status.success());
    let output = Command::new(&exe)
        .args(["launch", "--profile", "codex", "--topic", "Test launch"])
        .env("HERDR_ENV", "1")
        .env("HERDR_BIN_PATH", &herdr)
        .env("HERDR_FAKE_LOG", &log)
        .env("HERDR_INBOX_HOME", root.join("data"))
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let calls = fs::read_to_string(&log).unwrap();
    assert!(calls.contains("agent start spec_"));
    assert_eq!(calls.matches("agent start spec_").count(), 2);
    assert!(calls.contains("--kind codex"));
    assert!(calls.contains(
        "-m gpt-6.1-sol -c model_reasoning_effort=\"medium\" -s workspace-write --add-dir"
    ));
    assert!(calls.contains("$grill-me Test launch"));
    assert!(!calls.contains("--permission-mode"));
    let records: Value = serde_json::from_slice(
        &Command::new(exe)
            .args(["list", "--json"])
            .env("HERDR_INBOX_HOME", root.join("data"))
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert_eq!(records[0]["launch"]["harness"], "codex");
    assert_eq!(records[0]["launch"]["effort"], "medium");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_launch_that_never_opens_a_tab_leaves_no_item_behind() {
    let root = std::env::temp_dir().join(format!("herdr-inbox-launch-fail-{}", Uuid::new_v4()));
    let bin_dir = root.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let herdr = bin_dir.join("herdr-fake");
    fs::write(&herdr, r##"#!/bin/sh
case "$1 $2" in
  "workspace list") printf '%s\n' '{"result":{"workspaces":[{"label":"ai-boiler-room","workspace_id":"w2"}]}}' ;;
  "tab create") if [ -n "$HERDR_FAKE_TAB_FAILS" ]; then echo "tab limit reached" >&2; exit 1; fi
    printf '%s\n' '{"result":{"tab":{"tab_id":"w2:t2"},"root_pane":{"pane_id":"w2:p2"}}}' ;;
  "agent start") echo "agent refused to start" >&2; exit 1 ;;
  *) printf '%s\n' '{"result":{"type":"ok"}}' ;;
esac
"##).unwrap();
    fs::write(bin_dir.join("claude"), "#!/bin/sh\nexit 0\n").unwrap();
    for path in [&herdr, &bin_dir.join("claude")] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let path = format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap());
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_herdr-inbox"));
    let run = |args: &[&str], tab_fails: bool| {
        let mut command = Command::new(&exe);
        command
            .args(args)
            .env("HERDR_ENV", "1")
            .env("HERDR_BIN_PATH", &herdr)
            .env("HERDR_INBOX_HOME", root.join("data"))
            .env("PATH", &path);
        if tab_fails {
            command.env("HERDR_FAKE_TAB_FAILS", "1");
        }
        command.output().unwrap()
    };
    let records = |run: &dyn Fn(&[&str], bool) -> std::process::Output| -> Value {
        serde_json::from_slice(&run(&["list", "--json"], false).stdout).unwrap()
    };
    let source = root.join("chosen");
    fs::create_dir_all(&source).unwrap();
    assert!(
        run(&["settings", "add-source", source.to_str().unwrap()], false)
            .status
            .success()
    );

    // No tab: nothing to inspect, so retries must not pile up invisible items.
    for _ in 0..2 {
        let failed = run(&["launch"], true);
        assert!(!failed.status.success());
        assert!(String::from_utf8_lossy(&failed.stderr).contains("tab limit reached"));
        assert_eq!(records(&run).as_array().unwrap().len(), 0);
    }

    // A tab exists: the item stays, with the error recorded for inspection.
    let failed = run(&["launch"], false);
    assert!(!failed.status.success());
    let kept = records(&run);
    assert_eq!(kept.as_array().unwrap().len(), 1);
    assert_eq!(kept[0]["launch"]["status"], "failed");
    assert_eq!(kept[0]["launch"]["tab_id"], "w2:t2");
    assert!(
        kept[0]["launch"]["error"]
            .as_str()
            .unwrap()
            .contains("agent refused to start")
    );
    fs::remove_dir_all(root).unwrap();
}
