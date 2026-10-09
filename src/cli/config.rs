//! Commands that inspect and change this computer's configuration.

use super::{Args, Command, output};
use crate::launch::Profile;
use crate::settings::{Settings, SpecSource};
use crate::store::{Result, Store, absolute};
use std::path::PathBuf;

pub const COMMANDS: &[Command] = &[
    Command::new("settings", "settings ACTION (see below)", settings),
    Command::new("scan", "scan [--json]", scan),
    Command::new("path", "path", path),
];

/// A change to the settings file. Each runs under the store lock and is followed by a scan.
struct Action {
    name: &'static str,
    usage: &'static str,
    apply: fn(&mut Settings, Args) -> Result<()>,
}

const ACTIONS: &[Action] = &[
    Action {
        name: "add-source",
        usage: "add-source PATH [--flat] [--include GLOB]... [--exclude GLOB]...",
        apply: add_source,
    },
    Action {
        name: "remove-source",
        usage: "remove-source ID",
        apply: remove_source,
    },
    Action {
        name: "add-context",
        usage: "add-context PATH",
        apply: add_context,
    },
    Action {
        name: "remove-context",
        usage: "remove-context PATH",
        apply: remove_context,
    },
    Action {
        name: "defaults",
        usage: "defaults [--profile opus|codex] [--dev-skill NAME|none]",
        apply: defaults,
    },
];

pub fn actions_help() -> Vec<String> {
    let mut lines = vec![
        "settings show [--json]".to_owned(),
        "settings relocate-source ID PATH --confirm".to_owned(),
    ];
    lines.extend(
        ACTIONS
            .iter()
            .map(|action| format!("settings {}", action.usage)),
    );
    lines
}

fn settings(store: &Store, mut args: Args) -> Result<()> {
    let name = args.next().unwrap_or_else(|| "show".into());
    match name.as_str() {
        "show" => return show(store, args),
        "relocate-source" => return relocate_source(store, args),
        _ => {}
    }
    let action = ACTIONS
        .iter()
        .find(|action| action.name == name)
        .ok_or_else(|| format!("Unknown settings action: {name}"))?;
    // Read, change and write under one lock, so a concurrent Inbox or CLI update is never
    // overwritten with stale settings.
    store.update_settings(|settings| (action.apply)(settings, args))?;
    output::scan(&store.scan()?);
    Ok(())
}

fn show(store: &Store, mut args: Args) -> Result<()> {
    let json = args.switch("--json");
    let [] = args.positionals()?;
    let settings = store.settings()?;
    if json {
        return output::json(&settings);
    }
    println!("{}", store.settings_path().display());
    println!("{}", toml::to_string_pretty(&settings)?);
    Ok(())
}

fn relocate_source(store: &Store, mut args: Args) -> Result<()> {
    if !args.switch("--confirm") {
        return Err("Relocation requires --confirm; use add-source for unrelated content".into());
    }
    let [id, path] = args.positionals()?;
    output::scan(&store.relocate_source(&id, PathBuf::from(path))?);
    Ok(())
}

fn add_source(settings: &mut Settings, mut args: Args) -> Result<()> {
    let flat = args.switch("--flat");
    let includes = args.flags("--include")?;
    let excludes = args.flags("--exclude")?;
    let [path] = args.positionals()?;
    let mut source = SpecSource::new(PathBuf::from(path))?;
    source.recursive = !flat;
    if !includes.is_empty() {
        source.include = includes;
    }
    source.exclude = excludes;
    println!("Source: {}", source.id);
    settings.sources.push(source);
    Ok(())
}

fn remove_source(settings: &mut Settings, args: Args) -> Result<()> {
    let [id] = args.positionals()?;
    let count = settings.sources.len();
    settings.sources.retain(|source| source.id != id);
    if count == settings.sources.len() {
        return Err("Source ID not found".into());
    }
    Ok(())
}

fn add_context(settings: &mut Settings, args: Args) -> Result<()> {
    let [path] = args.positionals()?;
    let path = absolute(PathBuf::from(path))?;
    let metadata = std::fs::metadata(&path)?;
    if !(metadata.is_file() || metadata.is_dir()) {
        return Err("Context must be a file or directory".into());
    }
    if !settings.context_paths.contains(&path) {
        settings.context_paths.push(path);
    }
    Ok(())
}

fn remove_context(settings: &mut Settings, args: Args) -> Result<()> {
    let [path] = args.positionals()?;
    let path = absolute(PathBuf::from(path))?;
    let count = settings.context_paths.len();
    settings
        .context_paths
        .retain(|reference| *reference != path);
    if count == settings.context_paths.len() {
        return Err("Context reference not found".into());
    }
    Ok(())
}

fn defaults(settings: &mut Settings, mut args: Args) -> Result<()> {
    if let Some(profile) = args.flag("--profile")? {
        Profile::parse(&profile)?;
        settings.preferred_client = Some(profile);
    }
    if let Some(skill) = args.flag("--dev-skill")? {
        settings.dev_skill = (skill != "none").then_some(skill);
    }
    let [] = args.positionals()?;
    Ok(())
}

fn scan(store: &Store, mut args: Args) -> Result<()> {
    let json = args.switch("--json");
    let [] = args.positionals()?;
    let report = store.scan()?;
    if json {
        output::json(&report)
    } else {
        output::scan(&report);
        Ok(())
    }
}

fn path(store: &Store, args: Args) -> Result<()> {
    let [] = args.positionals()?;
    println!("{}", store.path().display());
    Ok(())
}
