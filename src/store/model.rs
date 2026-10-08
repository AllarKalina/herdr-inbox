use super::*;

/// A reference to external work: a Jira ticket or a draft PR.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Link<S> {
    pub status: S,
    pub key: Option<String>,
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
    pub source_id: Option<String>,
    pub source_relative_path: Option<PathBuf>,
    pub content_fingerprint: Option<String>,
    pub id: String,
    pub title: String,
    pub repo: Option<PathBuf>,
    pub spec_path: PathBuf,
    pub created_at: u64,
    pub updated_at: u64,
    pub spec: SpecStatus,
    pub jira: Link<JiraStatus>,
    pub implementation: Implementation,
    pub pr: Link<PrStatus>,
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
