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
mod archive;
mod config;
mod records;
mod scan;
pub use scan::ScanReport;
mod sources;
use sources::{fingerprint, fingerprint_bytes, same_file};

#[derive(Clone)]
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

    fn items_dir(&self) -> PathBuf {
        self.root.join("items")
    }

    fn archive_dir(&self) -> PathBuf {
        self.root.join("archive")
    }

    fn archived_path(&self, id: &str) -> Result<PathBuf> {
        Uuid::parse_str(id)?;
        Ok(self.archive_dir().join(format!("{id}.json")))
    }

    fn item_path(&self, id: &str) -> Result<PathBuf> {
        Uuid::parse_str(id)?;
        Ok(self.items_dir().join(format!("{id}.json")))
    }

    fn atomic_file(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        let parent = path.parent().ok_or("Invalid destination")?;
        ensure_dir(parent)?;
        let temporary = parent.join(format!(".{}.tmp", Uuid::new_v4()));
        let result = (|| -> Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            fs::rename(&temporary, path)?;
            fs::File::open(parent)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result
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

#[cfg(test)]
mod tests;
