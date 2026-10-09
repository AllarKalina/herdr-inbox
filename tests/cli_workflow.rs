//! End-to-end CLI regression: the whole spec-to-PR path through the real binary, with the
//! Jira stage on and off, plus the argument and prerequisite errors users actually hit.

mod common;

use common::Cli;
use std::fs;
use uuid::Uuid;

fn status_line(output: &str) -> &str {
    output.lines().nth(1).unwrap().trim()
}

fn next_line(output: &str) -> &str {
    output.lines().nth(2).unwrap().trim()
}

#[test]
fn full_pipeline_with_jira_enforces_each_prerequisite_in_order() {
    let cli = Cli::new("jira-on");
    let started = cli.ok(&["start", "Payment retries"]);
    assert_eq!(
        status_line(&started),
        "Spec: in_progress  Jira: waiting  Implementation: locked  PR: locked"
    );
    assert_eq!(next_line(&started), "Next: Finish spec");
    let id = cli.only_id();
    let id = id.as_str();

    assert!(
        cli.err(&["jira", id, "PAY-1"])
            .contains("Finish the spec before linking Jira")
    );
    assert!(
        cli.err(&["implement", id])
            .contains("link Jira before implementation")
    );
    assert!(
        cli.err(&["pr", id, "https://example.test/pull/1"])
            .contains("before a PR")
    );

    let finished = cli.ok(&["finish", id]);
    assert_eq!(
        status_line(&finished),
        "Spec: done  Jira: ready  Implementation: locked  PR: locked"
    );
    assert_eq!(next_line(&finished), "Next: Create Jira ticket");
    assert!(cli.err(&["finish", id]).contains("already finished"));
    assert!(
        cli.err(&["implement", id])
            .contains("link Jira before implementation")
    );

    let linked = cli.ok(&["jira", id, "PAY-1", "--url", "https://jira.test/PAY-1"]);
    assert_eq!(
        status_line(&linked),
        "Spec: done  Jira: created  Implementation: ready  PR: locked"
    );
    assert!(
        cli.err(&["pr", id, "https://example.test/pull/1"])
            .contains("before a PR")
    );

    let implementing = cli.ok(&[
        "implement",
        id,
        "--agent",
        "codex",
        "--branch",
        "feat/retries",
    ]);
    assert_eq!(
        status_line(&implementing),
        "Spec: done  Jira: created  Implementation: in_progress  PR: ready"
    );
    let drafted = cli.ok(&["pr", id, "https://example.test/pull/1"]);
    assert_eq!(
        status_line(&drafted),
        "Spec: done  Jira: created  Implementation: draft_pr  PR: draft"
    );
    assert_eq!(next_line(&drafted), "Next: Review draft PR");

    let record = cli.json(&["show", id, "--json"]);
    assert_eq!(record["title"], "Payment retries");
    assert_eq!(record["jira"]["key"], "PAY-1");
    assert_eq!(record["jira"]["url"], "https://jira.test/PAY-1");
    assert_eq!(record["implementation"]["agent"], "codex");
    assert_eq!(record["implementation"]["branch"], "feat/retries");
    assert_eq!(record["pr"]["url"], "https://example.test/pull/1");
    assert_eq!(record["schema_version"], 1);

    let retitled = cli.ok(&["title", id, "Payment retry handling"]);
    assert!(
        retitled
            .lines()
            .next()
            .unwrap()
            .ends_with("Payment retry handling")
    );
    assert!(
        cli.err(&["title", id, "  "])
            .contains("Title cannot be empty")
    );
}

#[test]
fn full_pipeline_without_jira_skips_the_ticket_and_hides_it_from_output() {
    let cli = Cli::new("jira-off");
    cli.ok(&["start", "Personal project"]);
    let id = cli.only_id();
    let id = id.as_str();
    cli.set_jira(false);

    let started = cli.ok(&["show", id]);
    assert_eq!(
        status_line(&started),
        "Spec: in_progress  Implementation: locked  PR: locked"
    );
    assert!(
        cli.err(&["implement", id])
            .contains("Finish the spec before implementation")
    );

    let finished = cli.ok(&["finish", id]);
    assert_eq!(
        status_line(&finished),
        "Spec: done  Implementation: ready  PR: locked"
    );
    assert_eq!(next_line(&finished), "Next: Hand spec to implementor");
    assert!(
        cli.err(&["jira", id, "PAY-1"])
            .contains("Jira is turned off in Settings")
    );
    assert!(
        cli.err(&["pr", id, "https://example.test/pull/2"])
            .contains("Start implementation before a PR")
    );

    cli.ok(&["implement", id, "--branch", "feat/personal"]);
    let drafted = cli.ok(&["pr", id, "https://example.test/pull/2"]);
    assert_eq!(
        status_line(&drafted),
        "Spec: done  Implementation: draft_pr  PR: draft"
    );
    assert!(!drafted.contains("Jira"));

    // The setting changes presentation and prerequisites, never the stored record.
    cli.set_jira(true);
    let shown = cli.ok(&["show", id]);
    assert_eq!(
        status_line(&shown),
        "Spec: done  Jira: ready  Implementation: draft_pr  PR: draft"
    );
    assert_eq!(
        cli.json(&["show", id, "--json"])["pr"]["url"],
        "https://example.test/pull/2"
    );
    // The ticket that was skipped can still be linked afterwards.
    let linked = cli.ok(&["jira", id, "LATE-1"]);
    assert!(status_line(&linked).contains("Jira: created"));
}

#[test]
fn archive_restore_and_scan_round_trip_through_the_cli() {
    let cli = Cli::new("archive");
    fs::write(cli.source.join("imported.md"), "# Imported plan\n").unwrap();
    let scan = cli.json(&["scan", "--json"]);
    assert_eq!(scan["imported"], 1);
    assert_eq!(scan["issues"].as_array().unwrap().len(), 0);
    let id = cli.only_id();
    let id = id.as_str();
    cli.ok(&["jira", id, "PLAN-7"]);

    assert!(cli.err(&["archive", id]).contains("--confirm"));
    assert!(
        cli.err(&["archive", id, "--confirm", &id[..8]])
            .contains("full matching item ID")
    );
    cli.ok(&["archive", id, "--confirm", id]);
    assert_eq!(cli.json(&["list", "--json"]).as_array().unwrap().len(), 0);
    let scan = cli.json(&["scan", "--json"]);
    assert_eq!(
        (scan["imported"].as_u64(), scan["archived"].as_u64()),
        (Some(0), Some(1))
    );

    let restored = cli.ok(&["restore", id]);
    assert!(restored.lines().next().unwrap().ends_with("Imported plan"));
    assert_eq!(cli.json(&["show", id, "--json"])["jira"]["key"], "PLAN-7");
    assert!(cli.err(&["restore", id]).contains("No such file"));
    assert!(cli.source.join("PLAN-7-imported-plan.md").is_file());
}

#[test]
fn help_lists_every_command_and_bad_invocations_fail_clearly() {
    let cli = Cli::new("usage");
    let help = cli.ok(&["help"]);
    for command in [
        "start",
        "launch",
        "ticket",
        "develop",
        "profiles",
        "settings show",
        "add-source",
        "remove-source",
        "relocate-source",
        "add-context",
        "remove-context",
        "defaults",
        "scan",
        "restore",
        "relink",
        "settle",
        "refine",
        "finish",
        "title",
        "archive",
        "jira",
        "implement",
        "pr",
        "list",
        "show",
        "path",
        "tui",
        "open",
    ] {
        assert!(help.contains(command), "help does not mention `{command}`");
    }
    assert_eq!(cli.ok(&[]), help);
    assert_eq!(cli.ok(&["--help"]), help);
    assert_eq!(cli.ok(&["path"]).trim(), cli.data.to_str().unwrap());

    assert!(cli.err(&["frobnicate"]).contains("Unknown command"));
    assert!(cli.err(&["show"]).contains("Wrong number of arguments"));
    assert!(
        cli.err(&["list", "extra"])
            .contains("Wrong number of arguments")
    );
    assert!(cli.err(&["start"]).contains("Wrong number of arguments"));
    assert!(
        cli.err(&["settings", "frobnicate"])
            .contains("Unknown settings action")
    );
    assert!(!cli.run(&["show", "not-a-uuid"]).status.success());
    assert!(
        !cli.run(&["show", &Uuid::new_v4().to_string()])
            .status
            .success()
    );

    // A settings file from another version stops every command instead of being overwritten.
    let settings = cli.data.join("settings.toml");
    let current = fs::read_to_string(&settings).unwrap();
    fs::write(&settings, current.replace("jira = true\n", "")).unwrap();
    assert!(cli.err(&["settings", "show"]).contains("jira"));
    assert!(cli.err(&["scan"]).contains("jira"));
    assert!(!fs::read_to_string(&settings).unwrap().contains("jira"));
}
