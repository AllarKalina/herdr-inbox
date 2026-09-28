mod store;
mod ui;

use std::path::PathBuf;
use std::process::Command;
use store::{Change, Record, Result, Store, git_root};

fn flag(args: &mut Vec<String>, name: &str) -> Result<Option<String>> {
    if let Some(index) = args.iter().position(|arg| arg == name) {
        args.remove(index);
        if index >= args.len() {
            return Err(format!("{name} needs a value").into());
        }
        Ok(Some(args.remove(index)))
    } else {
        Ok(None)
    }
}

fn positional(args: &[String], count: usize) -> Result<()> {
    if args.len() != count {
        return Err("Wrong number of arguments; run herdr-inbox help".into());
    }
    Ok(())
}

fn print_record(record: &Record) {
    println!("{}  {}", record.id, record.title);
    println!(
        "  Spec: {}  Jira: {}  Implementation: {}  PR: {}",
        record.spec, record.jira.status, record.implementation.status, record.pr.status
    );
    println!("  Next: {}", record.next_actions().join(", "));
    println!("  Spec file: {}", record.spec_path.display());
}

fn help() {
    println!("herdr-inbox — local spec-to-PR inbox");
    println!("  start TITLE [--repo PATH] [--spec PATH]");
    println!("  finish ID");
    println!("  jira ID KEY [--url URL]");
    println!("  implement ID [--agent NAME] [--branch BRANCH]");
    println!("  pr ID URL");
    println!("  list [--json] | show ID [--json] | path | tui | open");
}

fn run() -> Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "help" || args[0] == "--help" {
        help();
        return Ok(());
    }
    let command = args.remove(0);
    let store = Store::new(Store::default_path()?);
    match command.as_str() {
        "path" => {
            positional(&args, 0)?;
            println!("{}", store.path().display());
        }
        "start" => {
            let repo = flag(&mut args, "--repo")?.map(PathBuf::from).or_else(|| {
                if std::env::var_os("HERDR_PLUGIN_ID").is_some() {
                    None
                } else {
                    git_root()
                }
            });
            let spec = flag(&mut args, "--spec")?.map(PathBuf::from);
            positional(&args, 1)?;
            print_record(&store.start(&args[0], repo, spec)?);
        }
        "finish" => {
            positional(&args, 1)?;
            print_record(&store.update(&args[0], Change::Finish)?);
        }
        "jira" => {
            let url = flag(&mut args, "--url")?;
            positional(&args, 2)?;
            print_record(&store.update(
                &args[0],
                Change::Jira {
                    key: args[1].clone(),
                    url,
                },
            )?);
        }
        "implement" => {
            let agent = flag(&mut args, "--agent")?;
            let branch = flag(&mut args, "--branch")?;
            positional(&args, 1)?;
            print_record(&store.update(&args[0], Change::Implement { agent, branch })?);
        }
        "pr" => {
            positional(&args, 2)?;
            print_record(&store.update(
                &args[0],
                Change::Pr {
                    url: args[1].clone(),
                },
            )?);
        }
        "list" => {
            let json = args.first().is_some_and(|arg| arg == "--json");
            if json {
                args.remove(0);
            }
            positional(&args, 0)?;
            let records = store.list()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&records)?);
            } else {
                for record in records {
                    print_record(&record);
                    println!();
                }
            }
        }
        "show" => {
            let json = args.iter().any(|arg| arg == "--json");
            args.retain(|arg| arg != "--json");
            positional(&args, 1)?;
            let record = store.get(&args[0])?;
            if json {
                println!("{}", serde_json::to_string_pretty(&record)?);
            } else {
                print_record(&record);
            }
        }
        "tui" => {
            positional(&args, 0)?;
            ui::run(store)?;
        }
        "open" => {
            positional(&args, 0)?;
            let binary = std::env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".into());
            let status = Command::new(binary)
                .args([
                    "plugin",
                    "pane",
                    "open",
                    "--plugin",
                    "personal.inbox",
                    "--entrypoint",
                    "inbox",
                ])
                .status()?;
            if !status.success() {
                return Err("Could not open Herdr inbox pane".into());
            }
        }
        _ => return Err(format!("Unknown command: {command}").into()),
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("herdr-inbox: {error}");
        std::process::exit(1);
    }
}
