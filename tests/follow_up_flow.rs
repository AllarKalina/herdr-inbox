//! The sessions that follow a finished spec, against a fake `herdr`: an agent asked to
//! create the Jira ticket, the agent reporting back, and the development skill.

mod common;

use common::Cli;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

struct Herdr {
    cli: Cli,
    bin: PathBuf,
    log: PathBuf,
}

impl Herdr {
    /// `live_agents` are the agent names the fake reports as still running.
    fn new(name: &str) -> Self {
        let cli = Cli::new(name);
        let bin = cli.root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(
            bin.join("herdr-fake"),
            r##"#!/bin/sh
printf '%s\n' "$*" >> "$HERDR_FAKE_LOG"
case "$1 $2" in
  "workspace list") printf '%s\n' '{"result":{"workspaces":[{"label":"ai-boiler-room","workspace_id":"w2"}]}}' ;;
  "tab create") printf '%s\n' '{"result":{"tab":{"tab_id":"w2:t9"},"root_pane":{"pane_id":"w2:p9"}}}' ;;
  "agent get") case " $HERDR_FAKE_LIVE " in *" $3 "*) printf '%s\n' '{"result":{"agent":{}}}' ;; *) echo "agent_not_found" >&2; exit 1 ;; esac ;;
  *) printf '%s\n' '{"result":{"type":"ok"}}' ;;
esac
"##,
        )
        .unwrap();
        fs::write(bin.join("claude"), "#!/bin/sh\nexit 0\n").unwrap();
        for file in ["herdr-fake", "claude"] {
            fs::set_permissions(bin.join(file), fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self {
            log: cli.root.join("herdr.log"),
            bin,
            cli,
        }
    }

    fn run(&self, args: &[&str], live: &str) -> (bool, String, String) {
        let path = format!("{}:{}", self.bin.display(), std::env::var("PATH").unwrap());
        let output = Command::new(env!("CARGO_BIN_EXE_herdr-inbox"))
            .args(args)
            .env("HERDR_INBOX_HOME", &self.cli.data)
            .env("HERDR_ENV", "1")
            .env("HERDR_BIN_PATH", self.bin.join("herdr-fake"))
            .env("HERDR_FAKE_LOG", &self.log)
            .env("HERDR_FAKE_LIVE", live)
            .env("PATH", path)
            .output()
            .unwrap();
        (
            output.status.success(),
            String::from_utf8(output.stdout).unwrap(),
            String::from_utf8(output.stderr).unwrap(),
        )
    }

    fn ok(&self, args: &[&str], live: &str) -> String {
        let (success, stdout, stderr) = self.run(args, live);
        assert!(success, "{args:?} failed: {stderr}");
        stdout
    }

    /// The fake's call log since the last time it was read.
    fn calls(&self) -> String {
        let calls = fs::read_to_string(&self.log).unwrap_or_default();
        let _ = fs::remove_file(&self.log);
        calls
    }

    /// A finished spec; returns its item ID.
    fn finished_spec(&self) -> String {
        self.cli.spec(
            "retries.md",
            "# Payment retries\n\nRetry transient failures.\n",
        );
        self.cli.ok(&["scan"]);
        self.cli.only_id()
    }
}

#[test]
fn a_ticket_is_requested_from_a_new_session_and_linked_when_the_agent_reports_back() {
    let herdr = Herdr::new("ticket-new");
    let id = herdr.finished_spec();
    let requested = herdr.ok(&["ticket", &id, "--parent", "BT-2000"], "");
    assert!(requested.contains("Jira: requested"), "{requested}");
    assert!(requested.contains("Next: Await Jira ticket"), "{requested}");

    let calls = herdr.calls();
    assert!(
        calls.contains("tab create --workspace w2 --label Jira · Payment retries"),
        "{calls}"
    );
    assert!(calls.contains("--kind claude"), "{calls}");
    assert!(calls.contains("--permission-mode auto"), "{calls}");
    // The agent is told what to create, where, and exactly how to report it.
    assert!(
        calls.contains("Create a Jira issue for inbox item"),
        "{calls}"
    );
    assert!(calls.contains("Create it under BT-2000"), "{calls}");
    assert!(
        calls.contains(&format!("jira '{id}' <ISSUE-KEY> --url <issue URL>")),
        "{calls}"
    );
    let settings = herdr.cli.json(&["settings", "show", "--json"]);
    assert_eq!(
        settings["jira_parent"], "BT-2000",
        "the parent is offered again next time"
    );

    // What the agent runs once the issue exists.
    let linked = herdr
        .cli
        .ok(&["jira", &id, "BT-2300", "--url", "https://jira.test/BT-2300"]);
    assert!(linked.contains("Jira: created"), "{linked}");
    assert!(linked.contains("BT-2300-payment-retries.md"), "{linked}");
    assert!(
        herdr
            .cli
            .source
            .join("BT-2300-payment-retries.md")
            .is_file()
    );
    assert!(!herdr.cli.source.join("retries.md").exists());
}

#[test]
fn a_ticket_request_reuses_the_spec_session_while_its_claude_agent_is_running() {
    let herdr = Herdr::new("ticket-reuse");
    herdr.ok(&["launch", "--profile", "opus"], "");
    let id = herdr.cli.only_id();
    let record = herdr.cli.json(&["show", &id, "--json"]);
    let agent = record["launch"]["agent"].as_str().unwrap().to_owned();
    fs::write(
        record["spec_path"].as_str().unwrap(),
        "# Payment retries\n\nDone.\n",
    )
    .unwrap();
    herdr.cli.ok(&["finish", &id, "--title", "Payment retries"]);
    herdr.calls();

    herdr.ok(&["ticket", &id], &agent);
    let calls = herdr.calls();
    assert!(
        calls.contains(&format!("agent prompt {agent} Create a Jira issue")),
        "{calls}"
    );
    assert!(
        !calls.contains("tab create"),
        "the open session is reused: {calls}"
    );
    assert!(calls.contains("It has no parent issue"), "{calls}");

    // Once that tab is closed, the next request starts a session of its own.
    herdr.ok(&["ticket", &id], "");
    assert!(herdr.calls().contains("tab create"));
}

#[test]
fn ticket_requests_respect_the_workflow_and_the_jira_switch() {
    let herdr = Herdr::new("ticket-rules");
    herdr.cli.ok(&["start", "Unfinished"]);
    let id = herdr.cli.only_id();
    let (success, _, error) = herdr.run(&["ticket", &id], "");
    assert!(!success && error.contains("Finish the spec"), "{error}");
    herdr.cli.ok(&["finish", &id]);
    herdr.cli.set_jira(false);
    let (success, _, error) = herdr.run(&["ticket", &id], "");
    assert!(!success && error.contains("Jira is turned off"), "{error}");
    assert_eq!(
        herdr.calls(),
        "",
        "nothing is started for a refused request"
    );
}

#[test]
fn development_starts_the_configured_skill_with_the_ticket_key() {
    let herdr = Herdr::new("develop");
    let id = herdr.finished_spec();
    let (success, _, error) = herdr.run(&["develop", &id], "");
    assert!(
        !success && error.contains("No development skill is configured"),
        "{error}"
    );
    herdr
        .cli
        .ok(&["settings", "defaults", "--dev-skill", "/team-dev"]);
    let (success, _, error) = herdr.run(&["develop", &id], "");
    assert!(
        !success && error.contains("link Jira before implementation"),
        "{error}"
    );
    assert_eq!(herdr.calls(), "");

    herdr.cli.ok(&["jira", &id, "BT-2300"]);
    let started = herdr.ok(&["develop", &id], "");
    assert!(started.contains("Implementation: in_progress"), "{started}");
    let calls = herdr.calls();
    assert!(calls.contains("--label Dev · BT-2300"), "{calls}");
    assert!(calls.contains(" /team-dev BT-2300\n"), "{calls}");
    assert!(
        calls.contains(&format!(
            "implement '{id}' --agent 'team-dev' --branch <branch>"
        )),
        "{calls}"
    );
    assert!(calls.contains(&format!("pr '{id}' <PR URL>")), "{calls}");
    assert_eq!(
        herdr.cli.json(&["show", &id, "--json"])["implementation"]["agent"],
        "team-dev"
    );

    // What the skill runs as it goes.
    herdr.cli.ok(&[
        "implement",
        &id,
        "--agent",
        "team-dev",
        "--branch",
        "BT-2300-retries",
    ]);
    let drafted = herdr.cli.ok(&["pr", &id, "https://example.test/pull/9"]);
    assert!(drafted.contains("PR: draft"), "{drafted}");

    herdr
        .cli
        .ok(&["settings", "defaults", "--dev-skill", "none"]);
    assert!(herdr.cli.json(&["settings", "show", "--json"])["dev_skill"].is_null());
}
