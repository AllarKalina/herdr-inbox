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
mod discovery;
pub use discovery::ScanReport;
use discovery::{fingerprint, same_file};
mod lifecycle;

pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn default_path() -> Result<PathBuf> {
        if let Some(path) = std::env::var_os("HERDR_INBOX_HOME") {
            return expand_home(PathBuf::from(path));
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

    pub fn manages_spec(&self, record: &Record) -> bool {
        record.ownership == "managed"
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
                records.push(self.read_record(&path)?);
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
            match change {
                Change::Finish { title } => {
                    if record.spec != "in_progress" {
                        return Err("Spec is already finished".into());
                    }
                    if let Some(title) = title {
                        if title.trim().is_empty() {
                            return Err("Title cannot be empty".into());
                        }
                        record.title = title.trim().to_owned();
                    }
                    if record.title.is_empty() {
                        return Err("Give the spec a title before finishing".into());
                    }
                    if !record.spec_path.is_file() {
                        return Err("Spec file is missing".into());
                    }
                    record.spec = "done".into();
                    record.content_fingerprint = fingerprint(&record.spec_path).ok();
                    if let Some(launch) = &mut record.launch {
                        launch.status = "completed".into();
                    }
                    if record.jira.status == "waiting" {
                        record.jira.status = "ready".into();
                    }
                }
                Change::Jira { key, url } => {
                    if record.spec != "done" {
                        return Err("Finish the spec before linking Jira".into());
                    }
                    if key.trim().is_empty() {
                        return Err("Jira key cannot be empty".into());
                    }
                    record.jira.status = "created".into();
                    record.jira.key = Some(key.trim().to_owned());
                    record.jira.url = url;
                    if record.implementation.status == "waiting" {
                        record.implementation.status = "ready".into();
                    }
                    if record.implementation.status == "in_progress"
                        && record.pr.status == "waiting"
                    {
                        record.pr.status = "ready".into();
                    }
                }
                Change::Implement { agent, branch } => {
                    if record.spec != "done" || record.jira.status != "created" {
                        return Err("Finish the spec and link Jira before implementation".into());
                    }
                    if record.implementation.status != "draft_pr" {
                        record.implementation.status = "in_progress".into();
                    }
                    record.implementation.agent = agent;
                    record.implementation.branch = branch;
                    if record.pr.status == "waiting" {
                        record.pr.status = "ready".into();
                    }
                }
                Change::Pr { url } => {
                    if record.spec != "done"
                        || record.jira.status != "created"
                        || !matches!(
                            record.implementation.status.as_str(),
                            "in_progress" | "draft_pr"
                        )
                    {
                        return Err("Link Jira and start implementation before a PR".into());
                    }
                    record.implementation.status = "draft_pr".into();
                    record.pr.status = "draft".into();
                    record.pr.url = Some(url);
                }
                Change::Title { title } => {
                    if title.trim().is_empty() {
                        return Err("Title cannot be empty".into());
                    }
                    record.title = title.trim().to_owned();
                }
                Change::Launch(launch, expected_spec_path) => {
                    if record.spec_path != expected_spec_path {
                        return Err(
                            "Spec location changed during launch preflight; retry launch".into(),
                        );
                    }
                    record.launch = Some(*launch);
                }
                Change::BeginRefinement(launch, expected_spec_path) => {
                    if record.spec_path != expected_spec_path {
                        return Err(
                            "Spec location changed during launch preflight; retry refinement"
                                .into(),
                        );
                    }
                    if record.active_spec_session() {
                        return Err(
                            "Settle the active spec session before starting refinement".into()
                        );
                    }
                    if let Some(previous) = record.launch.take() {
                        record.previous_launches.push(previous);
                    }
                    record.launch = Some(*launch);
                }
                Change::RefineSpec => record.spec = "in_progress".into(),
            }
            record.updated_at = timestamp();
            self.write(&record)?;
            Ok(record)
        })
    }

    pub fn delete(&self, id: &str) -> Result<Record> {
        self.locked(|| {
            let record = self.get(id)?;
            let trash = self.root.join("trash");
            let item_trash = trash.join("items").join(format!("{id}.json"));
            let spec_trash = trash.join("specs").join(format!("{id}.md"));
            let managed_spec = self.manages_spec(&record) && record.spec_path.is_file();
            if item_trash.symlink_metadata().is_ok()
                || managed_spec && spec_trash.symlink_metadata().is_ok()
            {
                return Err("Trash already contains this item; restore or move it first".into());
            }
            ensure_dir(item_trash.parent().ok_or("Invalid trash item path")?)?;
            if managed_spec {
                ensure_dir(spec_trash.parent().ok_or("Invalid trash spec path")?)?;
                fs::rename(&record.spec_path, &spec_trash)?;
            }
            if let Err(error) = fs::rename(self.path_for(id)?, &item_trash) {
                if managed_spec {
                    let _ = fs::rename(&spec_trash, &record.spec_path);
                }
                return Err(error.into());
            }
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

pub enum Change {
    Finish {
        title: Option<String>,
    },
    Title {
        title: String,
    },
    Launch(Box<Launch>, PathBuf),
    BeginRefinement(Box<Launch>, PathBuf),
    RefineSpec,
    Jira {
        key: String,
        url: Option<String>,
    },
    Implement {
        agent: Option<String>,
        branch: Option<String>,
    },
    Pr {
        url: String,
    },
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

fn expand_home(path: PathBuf) -> Result<PathBuf> {
    if let Ok(rest) = path.strip_prefix("~") {
        let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
        return Ok(PathBuf::from(home).join(rest));
    }
    absolute(path)
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
