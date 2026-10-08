use crate::settings::SpecSource;
use crate::store::{Result, ScanReport, Store, absolute};
use crate::{flag, positional, print_record};
use std::path::PathBuf;

fn switch(args: &mut Vec<String>, name: &str) -> bool {
    if let Some(index) = args.iter().position(|arg| arg == name) {
        args.remove(index);
        true
    } else {
        false
    }
}

pub fn run(store: &Store, command: &str, mut args: Vec<String>) -> Result<bool> {
    match command {
        "scan" => {
            let json = switch(&mut args, "--json");
            positional(&args, 0)?;
            let report = store.scan()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_scan(&report);
            }
        }
        "restore" | "settle" => {
            positional(&args, 1)?;
            let record = if command == "restore" {
                store.restore(&args[0])?
            } else {
                store.settle(&args[0])?
            };
            print_record(store, &record);
        }
        "relink" => {
            positional(&args, 2)?;
            print_record(store, &store.relink(&args[0], PathBuf::from(&args[1]))?);
        }
        "settings" => settings(store, args)?,
        _ => return Ok(false),
    }
    Ok(true)
}

fn settings(store: &Store, mut args: Vec<String>) -> Result<()> {
    let action = if args.is_empty() {
        "show".into()
    } else {
        args.remove(0)
    };
    let mut settings = store.settings()?;
    match action.as_str() {
        "show" => {
            let json = switch(&mut args, "--json");
            positional(&args, 0)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&settings)?);
            } else {
                println!(
                    "{}\n{}",
                    store.settings_path().display(),
                    toml::to_string_pretty(&settings)?
                );
            }
            return Ok(());
        }
        "add-source" => {
            let flat = switch(&mut args, "--flat");
            let mut includes = Vec::new();
            let mut excludes = Vec::new();
            while let Some(pattern) = flag(&mut args, "--include")? {
                includes.push(pattern);
            }
            while let Some(pattern) = flag(&mut args, "--exclude")? {
                excludes.push(pattern);
            }
            positional(&args, 1)?;
            let mut source = SpecSource::new(PathBuf::from(&args[0]))?;
            source.recursive = !flat;
            if !includes.is_empty() {
                source.include = includes;
            }
            source.exclude = excludes;
            println!("Source: {}", source.id);
            settings.sources.push(source);
        }
        "remove-source" => {
            positional(&args, 1)?;
            let count = settings.sources.len();
            settings.sources.retain(|source| source.id != args[0]);
            if count == settings.sources.len() {
                return Err("Source ID not found".into());
            }
        }
        "relocate-source" => {
            if !switch(&mut args, "--confirm") {
                return Err(
                    "Relocation requires --confirm; use add-source for unrelated content".into(),
                );
            }
            positional(&args, 2)?;
            print_scan(&store.relocate_source(&args[0], PathBuf::from(&args[1]))?);
            return Ok(());
        }
        "add-context" | "remove-context" => {
            positional(&args, 1)?;
            let path = absolute(PathBuf::from(&args[0]))?;
            if action == "add-context" {
                let metadata = std::fs::metadata(&path)?;
                if !(metadata.is_file() || metadata.is_dir()) {
                    return Err("Context must be a file or directory".into());
                }
                if !settings.context_paths.contains(&path) {
                    settings.context_paths.push(path);
                }
            } else {
                let count = settings.context_paths.len();
                settings
                    .context_paths
                    .retain(|reference| *reference != path);
                if count == settings.context_paths.len() {
                    return Err("Context reference not found".into());
                }
            }
        }
        "defaults" => {
            if let Some(profile) = flag(&mut args, "--profile")? {
                crate::launch::Profile::parse(&profile)?;
                settings.preferred_client = Some(profile);
            }
            if let Some(workspace) = flag(&mut args, "--workspace")? {
                settings.workspace = workspace;
            }
            positional(&args, 0)?;
        }
        _ => return Err(format!("Unknown settings action: {action}").into()),
    }
    store.save_settings(&settings)?;
    print_scan(&store.scan()?);
    Ok(())
}

fn print_scan(report: &ScanReport) {
    println!("{}", report.summary());
    for issue in &report.issues {
        eprintln!("{issue}");
    }
}
