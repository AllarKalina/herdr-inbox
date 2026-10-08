pub use crate::settings::{Settings, SpecSource};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

mod model;
pub use model::*;
mod workflow;
pub use workflow::*;
mod discovery;
pub use discovery::ScanReport;
use discovery::{fingerprint, same_file};
mod lifecycle;
mod trash;

pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn default_path() -> Result<PathBuf> {
        if let Some(path) = std::env::var_os("HERDR_INBOX_HOME") {
            return absolute(PathBuf::from(path));
        }
        let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
        Ok(PathBuf::from(home).join("Library/Application Support/herdr-inbox"))
    }

    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    fn items(&self) -> PathBuf {
        self.root.join("items")
    }

    fn path_for(&self, id: &str) -> Result<PathBuf> {
        Uuid::parse_str(id)?;
        Ok(self.items().join(format!("{id}.json")))
    }

    fn locked<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        ensure_dir(&self.root)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(self.root.join(".lock"))?;
        lock.lock()?;
        let result = f();
        lock.unlock()?;
        result
    }

    pub fn start(
        &self,
        title: &str,
        repo: Option<PathBuf>,
        spec: Option<PathBuf>,
    ) -> Result<Record> {
        let title = title.trim();
        if title.is_empty() {
            return Err("Title cannot be empty".into());
        }
        self.create(title, repo, spec, true)
    }

    pub fn start_untitled(&self, repo: Option<PathBuf>) -> Result<Record> {
        self.start_untitled_with_spec(repo, None)
    }

    pub fn get(&self, id: &str) -> Result<Record> {
        let path = self.path_for(id)?;
        self.read_record(&path)
    }

    pub fn list(&self) -> Result<Vec<Record>> {
        let mut records = Vec::new();
        if !self.items().exists() {
            return Ok(records);
        }
        for entry in fs::read_dir(self.items())? {
            let path = entry?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                records.extend(self.read_listed(&path)?);
            }
        }
        records.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| b.id.cmp(&a.id))
        });
        Ok(records)
    }

    pub fn update(&self, id: &str, change: Change) -> Result<Record> {
        self.locked(|| {
            let mut record = self.get(id)?;
            record.apply(change, self.settings()?.jira)?;
            record.updated_at = timestamp();
            self.write(&record)?;
            Ok(record)
        })
    }

    pub fn archive(&self, id: &str) -> Result<Record> {
        self.locked(|| {
            let record = self.get(id)?;
            let trash = self.root.join("trash");
            let item_trash = trash.join("items").join(format!("{id}.json"));
            if item_trash.symlink_metadata().is_ok() {
                return Err("Archive already contains this item; restore it first".into());
            }
            ensure_dir(item_trash.parent().ok_or("Invalid archive item path")?)?;
            fs::rename(self.path_for(id)?, &item_trash)?;
            Ok(record)
        })
    }

    fn write(&self, record: &Record) -> Result<()> {
        if record.schema_version != 1 {
            return Err("Unsupported metadata schema; refusing to write".into());
        }
        let mut bytes = serde_json::to_vec_pretty(record)?;
        bytes.push(b'\n');
        self.atomic_file(&self.path_for(&record.id)?, &bytes)
    }
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn ensure_dir(path: &Path) -> io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
}

pub fn absolute(path: PathBuf) -> Result<PathBuf> {
    if let Ok(rest) = path.strip_prefix("~") {
        let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
        return Ok(PathBuf::from(home).join(rest));
    }
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

pub fn git_root() -> Option<PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if output.status.success() {
        Some(PathBuf::from(
            String::from_utf8_lossy(&output.stdout).trim(),
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
