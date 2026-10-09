//! The command line. Every command is one entry in a table: its name, its usage line, and
//! the function that runs it. Help and dispatch both read that table, so they cannot drift.

mod args;
mod config;
mod items;
mod output;
mod sessions;

use crate::store::{Result, Store};
pub use args::Args;

pub struct Command {
    name: &'static str,
    usage: &'static str,
    run: fn(&Store, Args) -> Result<()>,
}

impl Command {
    const fn new(
        name: &'static str,
        usage: &'static str,
        run: fn(&Store, Args) -> Result<()>,
    ) -> Self {
        Self { name, usage, run }
    }
}

fn commands() -> impl Iterator<Item = &'static Command> {
    [items::COMMANDS, sessions::COMMANDS, config::COMMANDS]
        .into_iter()
        .flatten()
}

fn help() {
    println!("herdr-inbox — local spec-to-PR inbox");
    for command in commands() {
        println!("  {}", command.usage);
    }
    println!();
    for line in config::actions_help() {
        println!("  {line}");
    }
    println!();
    println!("  {}", sessions::options_help());
}

pub fn run(mut args: Args) -> Result<()> {
    let name = args.next().unwrap_or_else(|| "help".into());
    if name == "help" || name == "--help" {
        help();
        return Ok(());
    }
    let command = commands()
        .find(|command| command.name == name)
        .ok_or_else(|| format!("Unknown command: {name}"))?;
    (command.run)(&Store::new(Store::default_path()?), args)
}
