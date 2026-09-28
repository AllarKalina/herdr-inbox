use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

#[test]
fn launches_untitled_spec_and_names_it_after_completion() {
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
    let repo = root.to_str().unwrap();
    let output = run(&[
        "launch",
        "--repo",
        repo,
        "--topic",
        "Improve payment retries",
    ]);
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
    assert!(
        calls.contains("--model claude-opus-5-5 --effort high --permission-mode bypassPermissions")
    );
    assert!(calls.contains("agent prompt spec_"));
    assert!(calls.contains("/grill-me Improve payment retries"));
    let spec = record["spec_path"].as_str().unwrap();
    fs::write(spec, "# Payment retries\n\nDetails.\n").unwrap();
    let id = record["id"].as_str().unwrap();
    let finish = run(&["finish", id, "--title", "Payment retries"]);
    assert!(
        finish.status.success(),
        "{}",
        String::from_utf8_lossy(&finish.stderr)
    );
    assert!(
        fs::read_to_string(&log)
            .unwrap()
            .contains("tab rename w2:t2 Payment retries")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn codex_profile_uses_sol_high_and_codex_skill() {
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
    let output = Command::new(&exe)
        .args([
            "launch",
            "--profile",
            "codex",
            "--repo",
            root.to_str().unwrap(),
            "--topic",
            "Test launch",
        ])
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
    assert!(
        calls.contains(
            "-m gpt-6-sol -c model_reasoning_effort=\"high\" -s workspace-write --add-dir"
        )
    );
    assert!(calls.contains("$grill-me Test launch"));
    assert!(!calls.contains("bypassPermissions"));
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
    assert_eq!(records[0]["launch"]["effort"], "high");
    fs::remove_dir_all(root).unwrap();
}
