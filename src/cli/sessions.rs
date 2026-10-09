//! Commands that start agent sessions in Herdr and open the Inbox itself.

use super::{Args, Command, output};
use crate::launch::{self, Options, Profile};
use crate::store::{Result, Store};

const SESSION_OPTIONS: &str =
    "[--profile opus|codex] [--model MODEL] [--effort LEVEL] [--topic TEXT] [--ask-permissions]";

pub const COMMANDS: &[Command] = &[
    Command::new("launch", "launch [session options]", start),
    Command::new("refine", "refine ID [session options]", refine),
    Command::new("ticket", "ticket ID [--parent KEY]", ticket),
    Command::new("develop", "develop ID", develop),
    Command::new("profiles", "profiles", profiles),
    Command::new("tui", "tui", tui),
    Command::new("open", "open", open),
];

pub fn options_help() -> String {
    format!("session options: {SESSION_OPTIONS}")
}

/// Session options start from the configured defaults; every flag overrides one of them.
fn options(store: &Store, args: &mut Args) -> Result<Options> {
    let settings = store.settings()?;
    let profile = args
        .flag("--profile")?
        .or(settings.preferred_client)
        .map(|value| Profile::parse(&value))
        .transpose()?
        .unwrap_or(Profile::Opus);
    let mut options = Options::for_profile(profile);
    if let Some(model) = args.flag("--model")? {
        options.model = model;
    }
    if let Some(effort) = args.flag("--effort")? {
        options.effort = effort;
    }
    if let Some(topic) = args.flag("--topic")? {
        options.topic = topic;
    }
    options.auto_permissions = !args.switch("--ask-permissions");
    Ok(options)
}

fn start(store: &Store, mut args: Args) -> Result<()> {
    let options = options(store, &mut args)?;
    let [] = args.positionals()?;
    output::record(store, &launch::start(store, options)?)
}

fn refine(store: &Store, mut args: Args) -> Result<()> {
    let options = options(store, &mut args)?;
    let [id] = args.positionals()?;
    output::record(store, &launch::refine(store, &id, options)?)
}

/// Asks an agent to create the item's Jira ticket and report it back.
fn ticket(store: &Store, mut args: Args) -> Result<()> {
    let parent = args.flag("--parent")?;
    let [id] = args.positionals()?;
    output::record(store, &launch::ticket(store, &id, parent)?)
}

/// Starts the configured development skill on the item.
fn develop(store: &Store, args: Args) -> Result<()> {
    let [id] = args.positionals()?;
    output::record(store, &launch::develop(store, &id)?)
}

fn profiles(_store: &Store, args: Args) -> Result<()> {
    let [] = args.positionals()?;
    for profile in launch::available_profiles() {
        println!("{}  {}", profile.id(), profile.label());
    }
    Ok(())
}

fn tui(store: &Store, args: Args) -> Result<()> {
    let [] = args.positionals()?;
    crate::ui::run(store.clone())
}

fn open(_store: &Store, args: Args) -> Result<()> {
    let [] = args.positionals()?;
    launch::open_inbox()
}
