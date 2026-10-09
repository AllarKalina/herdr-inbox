//! Commands that create, show, and advance Inbox items.

use super::{Args, Command, output};
use crate::launch;
use crate::store::{Change, Record, Result, Store};
use std::path::PathBuf;

pub const COMMANDS: &[Command] = &[
    Command::new("start", "start TITLE [--repo PATH] [--spec PATH]", start),
    Command::new("list", "list [--json]", list),
    Command::new("show", "show ID [--json]", show),
    Command::new("finish", "finish ID [--title TITLE]", finish),
    Command::new("title", "title ID TITLE", title),
    Command::new("jira", "jira ID KEY [--url URL]", jira),
    Command::new(
        "implement",
        "implement ID [--agent NAME] [--branch BRANCH]",
        implement,
    ),
    Command::new("pr", "pr ID URL", pr),
    Command::new("archive", "archive ID --confirm ID", archive),
    Command::new("restore", "restore ID", restore),
    Command::new("relink", "relink ID PATH", relink),
    Command::new("settle", "settle ID", settle),
];

fn start(store: &Store, mut args: Args) -> Result<()> {
    let repo = args.flag("--repo")?.map(PathBuf::from);
    let spec = args.flag("--spec")?.map(PathBuf::from);
    let [title] = args.positionals()?;
    output::record(store, &store.start(&title, repo, spec)?)
}

fn list(store: &Store, mut args: Args) -> Result<()> {
    let json = args.switch("--json");
    let [] = args.positionals()?;
    let records = store.list()?;
    if json {
        return output::json(&records);
    }
    for record in records {
        output::record(store, &record)?;
        println!();
    }
    Ok(())
}

fn show(store: &Store, mut args: Args) -> Result<()> {
    let json = args.switch("--json");
    let [id] = args.positionals()?;
    let record = store.get(&id)?;
    if json {
        output::json(&record)
    } else {
        output::record(store, &record)
    }
}

/// A finished or retitled spec renames its session tab; failing to is worth a note, no more.
fn renamed(store: &Store, record: Record) -> Result<()> {
    if let Err(error) = launch::rename_tab(&record) {
        eprintln!("Tab rename: {error}");
    }
    output::record(store, &record)
}

fn finish(store: &Store, mut args: Args) -> Result<()> {
    let title = args.flag("--title")?;
    let [id] = args.positionals()?;
    renamed(store, store.update(&id, Change::Finish { title })?)
}

fn title(store: &Store, args: Args) -> Result<()> {
    let [id, title] = args.positionals()?;
    renamed(store, store.update(&id, Change::Title { title })?)
}

fn jira(store: &Store, mut args: Args) -> Result<()> {
    let url = args.flag("--url")?;
    let [id, key] = args.positionals()?;
    output::record(store, &store.update(&id, Change::Jira { key, url })?)
}

fn implement(store: &Store, mut args: Args) -> Result<()> {
    let agent = args.flag("--agent")?;
    let branch = args.flag("--branch")?;
    let [id] = args.positionals()?;
    output::record(
        store,
        &store.update(&id, Change::Implement { agent, branch })?,
    )
}

fn pr(store: &Store, args: Args) -> Result<()> {
    let [id, url] = args.positionals()?;
    output::record(store, &store.update(&id, Change::Pr { url })?)
}

fn archive(store: &Store, mut args: Args) -> Result<()> {
    let confirmation = args.flag("--confirm")?;
    let [id] = args.positionals()?;
    if confirmation.as_deref() != Some(id.as_str()) {
        return Err("Archiving requires --confirm with the full matching item ID".into());
    }
    let record = store.archive(&id)?;
    println!("Archived {}", record.display_title());
    Ok(())
}

fn restore(store: &Store, args: Args) -> Result<()> {
    let [id] = args.positionals()?;
    output::record(store, &store.restore(&id)?)
}

fn relink(store: &Store, args: Args) -> Result<()> {
    let [id, path] = args.positionals()?;
    output::record(store, &store.relink(&id, PathBuf::from(path))?)
}

fn settle(store: &Store, args: Args) -> Result<()> {
    let [id] = args.positionals()?;
    output::record(store, &store.settle(&id)?)
}
