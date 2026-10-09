//! What the CLI prints for people; `--json` output is the stored data itself.

use crate::store::{Record, Result, ScanReport, Store};

pub fn record(store: &Store, record: &Record) -> Result<()> {
    let jira = store.settings()?.jira;
    println!("{}  {}", record.id, record.display_title());
    let jira_status = if jira {
        format!("Jira: {}  ", record.jira.status)
    } else {
        String::new()
    };
    println!(
        "  Spec: {}  {jira_status}Implementation: {}  PR: {}",
        record.spec,
        record.implementation_stage(jira),
        record.pr_stage(jira)
    );
    println!("  Next: {}", record.next_actions(jira).join(", "));
    println!("  Spec file: {}", record.spec_path.display());
    if let Some(launch) = &record.launch {
        println!(
            "  Session: {}  Client: {}  Model: {}  Effort: {}  Workspace: {}",
            launch.status, launch.harness, launch.model, launch.effort, launch.workspace
        );
        if let Some(error) = &launch.error {
            println!("  Launch error: {error}");
        }
    }
    Ok(())
}

pub fn scan(report: &ScanReport) {
    println!("{}", report.summary());
    for issue in &report.issues {
        eprintln!("{issue}");
    }
}

pub fn json(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
