//! Drives the real binary against a throwaway data directory.

// Each test binary uses only part of this.
#![allow(dead_code)]

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use uuid::Uuid;

pub struct Cli {
    pub root: PathBuf,
    pub data: PathBuf,
    pub source: PathBuf,
}

impl Cli {
    /// A fresh data directory with one selected, empty specs folder.
    pub fn new(name: &str) -> Self {
        let cli = Self::unconfigured(name);
        cli.ok(&["settings", "add-source", cli.source.to_str().unwrap()]);
        cli
    }

    /// A fresh data directory whose specs folder exists but is not selected yet.
    pub fn unconfigured(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("herdr-inbox-cli-{name}-{}", Uuid::new_v4()));
        let source = root.join("specs");
        fs::create_dir_all(&source).unwrap();
        Self {
            data: root.join("data"),
            root,
            source,
        }
    }

    pub fn spec(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.source.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }

    pub fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_herdr-inbox"))
            .args(args)
            .env("HERDR_INBOX_HOME", &self.data)
            .env("HERDR_ENV", "0")
            .env_remove("HERDR_PLUGIN_ID")
            .output()
            .unwrap()
    }

    pub fn ok(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    /// Runs a command that must fail and returns what it told the user.
    pub fn err(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(!output.status.success(), "{args:?} unexpectedly succeeded");
        String::from_utf8(output.stderr).unwrap()
    }

    pub fn json(&self, args: &[&str]) -> Value {
        serde_json::from_str(&self.ok(args)).unwrap()
    }

    pub fn only_id(&self) -> String {
        let records = self.json(&["list", "--json"]);
        assert_eq!(records.as_array().unwrap().len(), 1);
        records[0]["id"].as_str().unwrap().to_owned()
    }

    pub fn set_jira(&self, enabled: bool) {
        let path = self.data.join("settings.toml");
        let text = fs::read_to_string(&path).unwrap();
        let (from, to) = if enabled {
            ("jira = false", "jira = true")
        } else {
            ("jira = true", "jira = false")
        };
        assert!(
            text.contains(from),
            "settings.toml has no `{from}`:\n{text}"
        );
        fs::write(path, text.replace(from, to)).unwrap();
    }
}

impl Drop for Cli {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
