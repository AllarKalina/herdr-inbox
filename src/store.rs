use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Link {
    pub status: String,
    pub key: Option<String>,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Implementation {
    pub status: String,
    pub agent: Option<String>,
    pub branch: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub title: String,
    pub repo: Option<PathBuf>,
    pub spec_path: PathBuf,
    pub created_at: u64,
    pub updated_at: u64,
    pub spec: String,
    pub jira: Link,
    pub implementation: Implementation,
    pub pr: Link,
    #[serde(default)]
    pub launch: Option<Launch>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Launch {
    pub status: String,
    #[serde(default)]
    pub harness: String,
    pub workspace: String,
    pub workspace_id: Option<String>,
    pub tab_id: Option<String>,
    pub pane_id: Option<String>,
    pub agent: Option<String>,
    pub model: String,
    pub effort: String,
    pub prompt: String,
    pub error: Option<String>,
}

impl Record {
    pub fn display_title(&self) -> &str {
        if self.title.is_empty() {
            "Untitled spec"
        } else {
            &self.title
        }
    }

    pub fn next_actions(&self) -> Vec<&'static str> {
        if self
            .launch
            .as_ref()
            .is_some_and(|launch| launch.status == "failed")
        {
            return vec!["Inspect launch error"];
        }
        if self.spec == "in_progress" {
            return vec!["Finish spec"];
        }
        let mut actions = Vec::new();
        if self.jira.status == "ready" {
            actions.push("Create Jira ticket");
        }
        match self.implementation.status.as_str() {
            "ready" => actions.push("Hand spec to implementor"),
            "in_progress" => actions.push("Await draft PR"),
            "draft_pr" => actions.push("Review draft PR"),
            _ => {}
        }
        actions
    }
}

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
        self.create("", repo, None, false)
    }

    fn create(
        &self,
        title: &str,
        repo: Option<PathBuf>,
        spec: Option<PathBuf>,
        create_spec: bool,
    ) -> Result<Record> {
        self.locked(|| {
            let id = Uuid::new_v4().to_string();
            let path = match spec {
                Some(path) => absolute(path)?,
                None => self.root.join("specs").join(format!("{id}.md")),
            };
            if let Some(parent) = path.parent() {
                ensure_dir(parent)?;
            }
            if create_spec && !path.exists() {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&path)?;
                writeln!(file, "# {title}\n")?;
                file.sync_all()?;
            }
            let now = timestamp();
            let record = Record {
                id,
                title: title.to_owned(),
                repo: repo.map(absolute).transpose()?,
                spec_path: path,
                created_at: now,
                updated_at: now,
                spec: "in_progress".into(),
                jira: Link {
                    status: "waiting".into(),
                    key: None,
                    url: None,
                },
                implementation: Implementation {
                    status: "waiting".into(),
                    agent: None,
                    branch: None,
                },
                pr: Link {
                    status: "waiting".into(),
                    key: None,
                    url: None,
                },
                launch: None,
            };
            self.write(&record)?;
            Ok(record)
        })
    }

    pub fn get(&self, id: &str) -> Result<Record> {
        let path = self.path_for(id)?;
        Ok(serde_json::from_slice(&fs::read(path)?)?)
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
                records.push(serde_json::from_slice::<Record>(&fs::read(path)?)?);
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
                    record.jira.status = "ready".into();
                    record.implementation.status = "ready".into();
                }
                Change::Jira { key, url } => {
                    if record.spec != "done" {
                        return Err("Finish the spec before linking Jira".into());
                    }
                    record.jira.status = "created".into();
                    record.jira.key = Some(key);
                    record.jira.url = url;
                }
                Change::Implement { agent, branch } => {
                    if record.spec != "done" {
                        return Err("Finish the spec before implementation".into());
                    }
                    record.implementation.status = "in_progress".into();
                    record.implementation.agent = agent;
                    record.implementation.branch = branch;
                }
                Change::Pr { url } => {
                    if record.spec != "done" {
                        return Err("Finish the spec before linking a PR".into());
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
                Change::Launch(launch) => record.launch = Some(*launch),
            }
            record.updated_at = timestamp();
            self.write(&record)?;
            Ok(record)
        })
    }

    fn write(&self, record: &Record) -> Result<()> {
        ensure_dir(&self.items())?;
        let temporary = self.items().join(format!(".{}.tmp", Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        serde_json::to_writer_pretty(&mut file, record)?;
        writeln!(file)?;
        file.sync_all()?;
        fs::rename(temporary, self.path_for(&record.id)?)?;
        Ok(())
    }
}

pub enum Change {
    Finish {
        title: Option<String>,
    },
    Title {
        title: String,
    },
    Launch(Box<Launch>),
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
mod tests {
    use super::*;

    #[test]
    fn spec_to_pr_flow_keeps_independent_stage_statuses() -> Result<()> {
        let root = std::env::temp_dir().join(format!("herdr-inbox-test-{}", Uuid::new_v4()));
        let store = Store::new(root.clone());
        let started = store.start("Payment retries", None, None)?;
        assert!(started.spec_path.is_file());
        assert_eq!(started.next_actions(), vec!["Finish spec"]);
        assert!(
            store
                .update(
                    &started.id,
                    Change::Implement {
                        agent: None,
                        branch: None
                    }
                )
                .is_err()
        );
        assert!(
            store
                .update(
                    &started.id,
                    Change::Jira {
                        key: "ABC-123".into(),
                        url: None,
                    }
                )
                .is_err()
        );

        let finished = store.update(&started.id, Change::Finish { title: None })?;
        assert_eq!(
            finished.next_actions(),
            vec!["Create Jira ticket", "Hand spec to implementor"]
        );
        let implementing = store.update(
            &started.id,
            Change::Implement {
                agent: Some("codex".into()),
                branch: Some("feature/payment-retries".into()),
            },
        )?;
        assert_eq!(implementing.jira.status, "ready");
        assert_eq!(implementing.implementation.status, "in_progress");
        let linked = store.update(
            &started.id,
            Change::Jira {
                key: "ABC-123".into(),
                url: None,
            },
        )?;
        assert_eq!(linked.next_actions(), vec!["Await draft PR"]);
        let pr = store.update(
            &started.id,
            Change::Pr {
                url: "https://example.test/pr/1".into(),
            },
        )?;
        assert_eq!(pr.next_actions(), vec!["Review draft PR"]);
        assert_eq!(store.list()?.len(), 1);
        assert_eq!(fs::read_dir(root.join("items"))?.count(), 1);
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn existing_spec_is_preserved() -> Result<()> {
        let root = std::env::temp_dir().join(format!("herdr-inbox-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root)?;
        let spec = root.join("existing.md");
        fs::write(&spec, "Existing work\n")?;
        let store = Store::new(root.clone());
        let record = store.start("Existing", None, Some(spec.clone()))?;
        assert_eq!(fs::read_to_string(&spec)?, "Existing work\n");
        assert_eq!(record.spec_path, spec);
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn untitled_session_waits_for_written_spec_and_final_title() -> Result<()> {
        let root = std::env::temp_dir().join(format!("herdr-inbox-test-{}", Uuid::new_v4()));
        let store = Store::new(root.clone());
        let record = store.start_untitled(None)?;
        assert_eq!(record.display_title(), "Untitled spec");
        assert!(!record.spec_path.exists());
        assert!(
            store
                .update(
                    &record.id,
                    Change::Finish {
                        title: Some("Final name".into())
                    }
                )
                .is_err()
        );
        fs::write(&record.spec_path, "# Final name\n\nReal spec\n")?;
        let done = store.update(
            &record.id,
            Change::Finish {
                title: Some("Final name".into()),
            },
        )?;
        assert_eq!(done.title, "Final name");
        assert_eq!(done.spec, "done");
        fs::remove_dir_all(root)?;
        Ok(())
    }
}
