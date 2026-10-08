//! What the user is in the middle of: an open prompt, a client choice, the action
//! Enter would run, and the acknowledgement shown after a step completes.

use super::Milestone;
use crate::launch::Profile;
use std::path::PathBuf;

#[derive(Clone)]
pub(super) enum Prompt {
    LaunchWorkspace {
        profile: Profile,
    },
    LaunchRepo {
        profile: Profile,
        workspace: String,
    },
    LaunchSpec {
        profile: Profile,
        workspace: String,
        repo: Option<PathBuf>,
    },
    LaunchTopic {
        profile: Profile,
        workspace: String,
        repo: Option<PathBuf>,
        spec: Option<PathBuf>,
    },
    Settle {
        id: String,
    },
    FinishTitle {
        id: String,
    },
    Jira {
        id: String,
    },
    JiraUrl {
        id: String,
        key: String,
    },
    Agent {
        id: String,
    },
    Branch {
        id: String,
        agent: Option<String>,
    },
    Pr {
        id: String,
    },
    Archive {
        id: String,
    },
}

impl Prompt {
    /// The item this prompt acts on; launch prompts create a new one.
    pub(super) fn item(&self) -> Option<&str> {
        match self {
            Self::Settle { id }
            | Self::FinishTitle { id }
            | Self::Jira { id }
            | Self::JiraUrl { id, .. }
            | Self::Agent { id }
            | Self::Branch { id, .. }
            | Self::Pr { id }
            | Self::Archive { id } => Some(id),
            Self::LaunchWorkspace { .. }
            | Self::LaunchRepo { .. }
            | Self::LaunchSpec { .. }
            | Self::LaunchTopic { .. } => None,
        }
    }

    pub(super) fn label(&self) -> &'static str {
        match self {
            Self::LaunchWorkspace { .. } => "Herdr workspace",
            Self::LaunchSpec { .. } => "Spec path (blank = first source folder)",
            Self::Settle { .. } => "Confirm session has ended",
            Self::LaunchRepo { .. } => "Repo path (blank for workspace cwd)",
            Self::LaunchTopic { .. } => "Grilling topic (optional)",
            Self::FinishTitle { .. } => "Finished spec title",
            Self::Jira { .. } => "Jira key",
            Self::JiraUrl { .. } => "Jira URL (optional)",
            Self::Agent { .. } => "Agent name (optional)",
            Self::Branch { .. } => "Branch (optional)",
            Self::Pr { .. } => "Draft PR URL",
            Self::Archive { .. } => "Confirm archive",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ChoicePurpose {
    NewSpec,
    Refine { id: String },
}

pub(super) struct MilestoneFeedback {
    pub(super) milestone: Milestone,
}

impl MilestoneFeedback {
    pub(super) fn text(&self, compact: bool) -> &'static str {
        match (self.milestone, compact) {
            (Milestone::Spec, false) => "Spec sealed",
            (Milestone::Jira, false) => "Jira bound",
            (Milestone::Dev, false) => "Dev quest logged",
            (Milestone::Pr, false) => "Draft PR bound",
            (Milestone::Spec, true) => "Sealed",
            (Milestone::Jira | Milestone::Pr, true) => "Bound",
            (Milestone::Dev, true) => "Logged",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DetailAction {
    Finish,
    Jira,
    Implement,
    Pr,
    ReviewPr,
    ReadSpec,
    RefineSpec,
    OpenJira,
    UpdateJira,
    UpdateImplementation,
    UpdatePr,
}

impl DetailAction {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Finish => "Seal the spec",
            Self::Jira => "Bind Jira ticket",
            Self::Implement => "Log dev quest",
            Self::Pr => "Bind draft PR",
            Self::ReviewPr => "Review draft PR",
            Self::ReadSpec => "Read the scroll",
            Self::RefineSpec => "Refine the spec",
            Self::OpenJira => "Visit Jira ticket",
            Self::UpdateJira => "Update Jira link",
            Self::UpdateImplementation => "Update dev quest",
            Self::UpdatePr => "Update PR link",
        }
    }
}
