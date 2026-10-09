//! What the user is in the middle of: an open prompt, the action Enter would run, the
//! acknowledgement shown after a step completes, and the one-line notice above the shortcuts.

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

    /// A yes-or-no question: Enter confirms and typing does nothing.
    pub(super) fn is_confirmation(&self) -> bool {
        matches!(self, Self::Archive { .. } | Self::Settle { .. })
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tone {
    Info,
    Success,
    Error,
}

/// The one-line message shown above the shortcut line until the next action replaces it.
pub(super) struct Notice {
    text: String,
    tone: Tone,
}

impl Default for Notice {
    fn default() -> Self {
        Self {
            text: String::new(),
            tone: Tone::Info,
        }
    }
}

impl Notice {
    fn set(&mut self, text: impl Into<String>, tone: Tone) {
        self.text = text.into();
        self.tone = tone;
    }

    pub(super) fn info(&mut self, text: impl Into<String>) {
        self.set(text, Tone::Info);
    }

    pub(super) fn success(&mut self, text: impl Into<String>) {
        self.set(text, Tone::Success);
    }

    pub(super) fn error(&mut self, text: impl Into<String>) {
        self.set(text, Tone::Error);
    }

    pub(super) fn clear(&mut self) {
        self.text.clear();
    }

    pub(super) fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }

    pub(super) fn tone(&self) -> Tone {
        self.tone
    }
}

/// A vertical scroll position that never passes the end of its content.
#[derive(Default)]
pub(super) struct Scroll {
    offset: u16,
    max: u16,
}

impl Scroll {
    pub(super) fn offset(&self) -> u16 {
        self.offset
    }

    pub(super) fn down(&mut self, lines: u16) {
        self.offset = self.offset.saturating_add(lines).min(self.max);
    }

    pub(super) fn up(&mut self, lines: u16) {
        self.offset = self.offset.saturating_sub(lines);
    }

    pub(super) fn top(&mut self) {
        self.offset = 0;
    }

    /// Called while drawing, once the content's height in the current layout is known.
    pub(super) fn limit(&mut self, max: usize) {
        self.max = max.min(usize::from(u16::MAX)) as u16;
        self.offset = self.offset.min(self.max);
    }
}

#[cfg(test)]
mod tests {
    use super::Scroll;

    #[test]
    fn scroll_stays_between_the_top_and_the_end_of_its_content() {
        let mut scroll = Scroll::default();
        scroll.down(5);
        assert_eq!(
            scroll.offset(),
            0,
            "nothing to scroll before content is measured"
        );
        scroll.limit(12);
        scroll.down(10);
        scroll.down(10);
        assert_eq!(scroll.offset(), 12);
        scroll.up(3);
        assert_eq!(scroll.offset(), 9);
        // Content that shrinks, or a taller viewport, pulls the position back in range.
        scroll.limit(4);
        assert_eq!(scroll.offset(), 4);
        scroll.up(10);
        assert_eq!(scroll.offset(), 0);
        scroll.down(2);
        scroll.top();
        assert_eq!(scroll.offset(), 0);
        scroll.limit(usize::MAX);
        scroll.down(u16::MAX);
        assert_eq!(scroll.offset(), u16::MAX);
    }
}
