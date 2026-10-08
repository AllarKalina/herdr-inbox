use super::*;

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
    pub spec: String,
    pub jira: Link,
    pub implementation: Implementation,
    pub pr: Link,
    pub launch: Option<Launch>,
    pub previous_launches: Vec<Launch>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Launch {
    pub status: String,
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
    pub fn active_spec_session(&self) -> bool {
        self.launch.as_ref().is_some_and(|launch| {
            !matches!(launch.status.as_str(), "failed" | "completed" | "settled")
        })
    }

    /// True for a record that a scan created and nobody has worked on since.
    pub fn untouched_import(&self) -> bool {
        self.spec == "done"
            && self.jira.status == "ready"
            && self.jira.key.is_none()
            && self.implementation.status == "waiting"
            && self.implementation.agent.is_none()
            && self.implementation.branch.is_none()
            && self.pr.status == "waiting"
            && self.pr.url.is_none()
            && self.repo.is_none()
            && self.launch.is_none()
            && self.previous_launches.is_empty()
    }

    pub fn display_title(&self) -> &str {
        if self.title.is_empty() {
            "Untitled spec"
        } else {
            &self.title
        }
    }

    /// With the Jira stage turned off, a finished spec is the only prerequisite.
    fn jira_satisfied(&self, jira: bool) -> bool {
        !jira || self.jira.status == "created"
    }

    pub fn implementation_stage(&self, jira: bool) -> &str {
        match self.implementation.status.as_str() {
            "waiting" | "ready" if self.spec == "done" && self.jira_satisfied(jira) => "ready",
            "waiting" | "ready" => "locked",
            status => status,
        }
    }

    pub fn pr_stage(&self, jira: bool) -> &str {
        match self.pr.status.as_str() {
            "waiting" | "ready"
                if self.spec == "done"
                    && self.jira_satisfied(jira)
                    && self.implementation.status == "in_progress" =>
            {
                "ready"
            }
            "waiting" | "ready" => "locked",
            status => status,
        }
    }

    pub fn next_actions(&self, jira: bool) -> Vec<&'static str> {
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
        if jira && self.jira.status == "ready" {
            actions.push("Create Jira ticket");
            return actions;
        }
        match self.implementation_stage(jira) {
            "ready" => actions.push("Hand spec to implementor"),
            "in_progress" => actions.push("Await draft PR"),
            "draft_pr" => actions.push("Review draft PR"),
            _ => {}
        }
        actions
    }
}
