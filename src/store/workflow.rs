//! The spec → Jira → implementation → draft PR workflow: every status a record can hold and
//! every rule about moving between them. The UI and CLI only present what this module decides.

use super::*;

/// Declares a status whose variants are stored and printed under fixed snake_case names.
macro_rules! status {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name {
            $(#[serde(rename = $text)] $variant),+
        }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        /// Tests compare against the stored name; production code matches on the variant.
        #[cfg(test)]
        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.as_str() == *other
            }
        }
    };
}

status!(SpecStatus { InProgress => "in_progress", Done => "done" });
status!(JiraStatus {
    Waiting => "waiting",
    Ready => "ready",
    // An agent has been asked to create the ticket and has not reported back yet.
    Requested => "requested",
    Created => "created",
});
status!(ImplementationStatus {
    Waiting => "waiting",
    Ready => "ready",
    InProgress => "in_progress",
    DraftPr => "draft_pr",
});
status!(PrStatus { Waiting => "waiting", Ready => "ready", Draft => "draft" });
status!(
    /// How far the local launch of a spec session got. Nothing here tracks the agent itself.
    LaunchStatus {
        Starting => "starting",
        TabOpened => "tab_opened",
        AgentStarted => "agent_started",
        PromptSent => "prompt_sent",
        Failed => "failed",
        Completed => "completed",
    }
);
status!(
    /// Where implementation stands once its prerequisites are taken into account.
    ImplementationStage {
        Locked => "locked",
        Ready => "ready",
        InProgress => "in_progress",
        DraftPr => "draft_pr",
    }
);
status!(
    /// Where the draft PR stands once its prerequisites are taken into account.
    PrStage { Locked => "locked", Ready => "ready", Draft => "draft" }
);

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
    /// An agent was asked to create the ticket.
    RequestJira,
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

impl Record {
    /// A record before any workflow step: the starting point for new and imported specs alike.
    pub(crate) fn new(id: String, location: Location, spec: SpecStatus, now: u64) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            source_id: location.source_id,
            source_relative_path: location.relative,
            content_fingerprint: None,
            id,
            title: String::new(),
            repo: None,
            spec_path: location.path,
            created_at: now,
            updated_at: now,
            spec,
            jira: Jira {
                status: match spec {
                    SpecStatus::Done => JiraStatus::Ready,
                    SpecStatus::InProgress => JiraStatus::Waiting,
                },
                key: None,
                url: None,
            },
            implementation: Implementation {
                status: ImplementationStatus::Waiting,
                agent: None,
                branch: None,
            },
            pr: PullRequest {
                status: PrStatus::Waiting,
                url: None,
            },
            launch: None,
            previous_launches: Vec::new(),
        }
    }

    /// Points the record at a spec file in a selected folder.
    pub(super) fn place(&mut self, location: Location) {
        self.source_id = location.source_id;
        self.source_relative_path = location.relative;
        self.spec_path = location.path;
    }

    pub fn active_spec_session(&self) -> bool {
        self.launch.as_ref().is_some_and(|launch| {
            !matches!(
                launch.status,
                LaunchStatus::Failed | LaunchStatus::Completed
            )
        })
    }

    /// True for a record that a scan created and nobody has worked on since.
    pub fn untouched_import(&self) -> bool {
        self.spec == SpecStatus::Done
            && self.jira.status == JiraStatus::Ready
            && self.jira.key.is_none()
            && self.implementation.status == ImplementationStatus::Waiting
            && self.implementation.agent.is_none()
            && self.implementation.branch.is_none()
            && self.pr.status == PrStatus::Waiting
            && self.pr.url.is_none()
            && self.repo.is_none()
            && self.launch.is_none()
            && self.previous_launches.is_empty()
    }

    /// With the Jira stage turned off, a finished spec is the only prerequisite.
    fn ticket_satisfied(&self, jira: bool) -> bool {
        !jira || self.jira.status == JiraStatus::Created
    }

    fn implementation_started(&self) -> bool {
        matches!(
            self.implementation.status,
            ImplementationStatus::InProgress | ImplementationStatus::DraftPr
        )
    }

    pub fn implementation_stage(&self, jira: bool) -> ImplementationStage {
        match self.implementation.status {
            ImplementationStatus::InProgress => ImplementationStage::InProgress,
            ImplementationStatus::DraftPr => ImplementationStage::DraftPr,
            ImplementationStatus::Waiting | ImplementationStatus::Ready
                if self.spec == SpecStatus::Done && self.ticket_satisfied(jira) =>
            {
                ImplementationStage::Ready
            }
            ImplementationStatus::Waiting | ImplementationStatus::Ready => {
                ImplementationStage::Locked
            }
        }
    }

    pub fn pr_stage(&self, jira: bool) -> PrStage {
        match self.pr.status {
            PrStatus::Draft => PrStage::Draft,
            PrStatus::Waiting | PrStatus::Ready
                if self.spec == SpecStatus::Done
                    && self.ticket_satisfied(jira)
                    && self.implementation.status == ImplementationStatus::InProgress =>
            {
                PrStage::Ready
            }
            PrStatus::Waiting | PrStatus::Ready => PrStage::Locked,
        }
    }

    pub fn next_actions(&self, jira: bool) -> Vec<&'static str> {
        if self
            .launch
            .as_ref()
            .is_some_and(|launch| launch.status == LaunchStatus::Failed)
        {
            return vec!["Inspect launch error"];
        }
        if self.spec == SpecStatus::InProgress {
            return vec!["Finish spec"];
        }
        match self.jira.status {
            JiraStatus::Ready if jira => return vec!["Create Jira ticket"],
            JiraStatus::Requested if jira => return vec!["Await Jira ticket"],
            _ => {}
        }
        match self.implementation_stage(jira) {
            ImplementationStage::Ready => vec!["Hand spec to implementor"],
            ImplementationStage::InProgress => vec!["Await draft PR"],
            ImplementationStage::DraftPr => vec!["Review draft PR"],
            ImplementationStage::Locked => Vec::new(),
        }
    }

    /// Checks that a step would be accepted, without taking it.
    pub fn permits(&self, change: Change, jira: bool) -> Result<()> {
        self.clone().apply(change, jira)
    }

    /// Applies one workflow step, or explains which prerequisite is missing.
    pub(super) fn apply(&mut self, change: Change, jira: bool) -> Result<()> {
        match change {
            Change::Finish { title } => {
                if self.spec != SpecStatus::InProgress {
                    return Err("Spec is already finished".into());
                }
                if let Some(title) = title {
                    if title.trim().is_empty() {
                        return Err("Title cannot be empty".into());
                    }
                    self.title = title.trim().to_owned();
                }
                if self.title.is_empty() {
                    return Err("Give the spec a title before finishing".into());
                }
                if !self.spec_path.is_file() {
                    return Err("Spec file is missing".into());
                }
                self.spec = SpecStatus::Done;
                self.content_fingerprint = fingerprint(&self.spec_path).ok();
                if let Some(launch) = &mut self.launch {
                    launch.status = LaunchStatus::Completed;
                }
                if self.jira.status == JiraStatus::Waiting {
                    self.jira.status = JiraStatus::Ready;
                }
            }
            Change::RequestJira => {
                if !jira {
                    return Err("Jira is turned off in Settings".into());
                }
                if self.spec != SpecStatus::Done {
                    return Err("Finish the spec before creating a Jira ticket".into());
                }
                if self.jira.status != JiraStatus::Created {
                    self.jira.status = JiraStatus::Requested;
                }
            }
            Change::Jira { key, url } => {
                if !jira {
                    return Err("Jira is turned off in Settings".into());
                }
                if self.spec != SpecStatus::Done {
                    return Err("Finish the spec before linking Jira".into());
                }
                if key.trim().is_empty() {
                    return Err("Jira key cannot be empty".into());
                }
                self.jira.status = JiraStatus::Created;
                self.jira.key = Some(key.trim().to_owned());
                self.jira.url = url;
                if self.implementation.status == ImplementationStatus::Waiting {
                    self.implementation.status = ImplementationStatus::Ready;
                }
                if self.implementation.status == ImplementationStatus::InProgress
                    && self.pr.status == PrStatus::Waiting
                {
                    self.pr.status = PrStatus::Ready;
                }
            }
            Change::Implement { agent, branch } => {
                if self.spec != SpecStatus::Done || !self.ticket_satisfied(jira) {
                    return Err(if jira {
                        "Finish the spec and link Jira before implementation"
                    } else {
                        "Finish the spec before implementation"
                    }
                    .into());
                }
                if self.implementation.status != ImplementationStatus::DraftPr {
                    self.implementation.status = ImplementationStatus::InProgress;
                }
                self.implementation.agent = agent;
                self.implementation.branch = branch;
                if self.pr.status == PrStatus::Waiting {
                    self.pr.status = PrStatus::Ready;
                }
            }
            Change::Pr { url } => {
                if self.spec != SpecStatus::Done
                    || !self.ticket_satisfied(jira)
                    || !self.implementation_started()
                {
                    return Err(if jira {
                        "Link Jira and start implementation before a PR"
                    } else {
                        "Start implementation before a PR"
                    }
                    .into());
                }
                self.implementation.status = ImplementationStatus::DraftPr;
                self.pr.status = PrStatus::Draft;
                self.pr.url = Some(url);
            }
            Change::Title { title } => {
                if title.trim().is_empty() {
                    return Err("Title cannot be empty".into());
                }
                self.title = title.trim().to_owned();
            }
            Change::Launch(launch, expected_spec_path) => {
                if self.spec_path != expected_spec_path {
                    return Err(
                        "Spec location changed during launch preflight; retry launch".into(),
                    );
                }
                self.launch = Some(*launch);
            }
            Change::BeginRefinement(launch, expected_spec_path) => {
                if self.spec_path != expected_spec_path {
                    return Err(
                        "Spec location changed during launch preflight; retry refinement".into(),
                    );
                }
                if self.active_spec_session() {
                    return Err("Settle the active spec session before starting refinement".into());
                }
                if let Some(previous) = self.launch.take() {
                    self.previous_launches.push(previous);
                }
                self.launch = Some(*launch);
            }
            Change::RefineSpec => self.spec = SpecStatus::InProgress,
        }
        Ok(())
    }
}
