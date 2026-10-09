use super::*;

/// The only metadata schema this version reads or writes.
pub const SCHEMA_VERSION: u32 = 1;

/// Where a spec lives: its selected folder, its path inside that folder, and its resolved path.
pub struct Location {
    pub source_id: String,
    pub relative: PathBuf,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Jira {
    pub status: JiraStatus,
    pub key: Option<String>,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PullRequest {
    pub status: PrStatus,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Implementation {
    pub status: ImplementationStatus,
    pub agent: Option<String>,
    pub branch: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub schema_version: u32,
    pub source_id: String,
    pub source_relative_path: PathBuf,
    pub content_fingerprint: Option<String>,
    pub id: String,
    pub title: String,
    pub spec_path: PathBuf,
    pub created_at: u64,
    pub updated_at: u64,
    pub spec: SpecStatus,
    pub jira: Jira,
    pub implementation: Implementation,
    pub pr: PullRequest,
    pub launch: Option<Launch>,
    pub previous_launches: Vec<Launch>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Launch {
    pub status: LaunchStatus,
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
}
