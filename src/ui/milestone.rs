use super::DetailAction;
use crate::store::{ImplementationStage, JiraStatus, PrStage, Record, SpecStatus};
use ratatui::style::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Milestone {
    Spec,
    Jira,
    Dev,
    Pr,
}

/// How a milestone reads at a glance, whichever stage it belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StageState {
    Locked,
    Waiting,
    Ready,
    Active,
    Done,
    Draft,
}

impl StageState {
    pub(super) fn color(self) -> Color {
        match self {
            Self::Locked => Color::DarkGray,
            Self::Waiting => Color::Gray,
            Self::Ready => Color::LightBlue,
            Self::Active => Color::Cyan,
            Self::Done => Color::LightGreen,
            Self::Draft => Color::Yellow,
        }
    }

    pub(super) fn word(self) -> &'static str {
        match self {
            Self::Locked => "locked",
            Self::Waiting => "wait",
            Self::Ready => "ready",
            Self::Active => "active",
            Self::Done => "done",
            Self::Draft => "draft",
        }
    }

    /// The icon beside the word in an Inbox list column.
    pub(super) fn list_icon(self) -> &'static str {
        match self {
            Self::Locked | Self::Waiting => "○",
            Self::Ready => "→",
            Self::Active => "●",
            Self::Done => "✓",
            Self::Draft => "◐",
        }
    }

    /// The node drawn on the progress rail's spine.
    pub(super) fn node(self) -> &'static str {
        match self {
            Self::Locked | Self::Waiting | Self::Ready => "○",
            Self::Active => "◉",
            Self::Done => "●",
            Self::Draft => "◐",
        }
    }
}

impl Milestone {
    pub(super) const ALL: [Self; 4] = [Self::Spec, Self::Jira, Self::Dev, Self::Pr];
    const WITHOUT_JIRA: [Self; 3] = [Self::Spec, Self::Dev, Self::Pr];

    /// The stages shown on this computer; Jira disappears when it is turned off in Settings.
    pub(super) fn visible(jira: bool) -> &'static [Self] {
        if jira {
            &Self::ALL
        } else {
            &Self::WITHOUT_JIRA
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Spec => "SPEC",
            Self::Jira => "JIRA",
            Self::Dev => "DEV",
            Self::Pr => "PR",
        }
    }

    /// The stage's column heading in the Inbox list.
    pub(super) fn column(self) -> &'static str {
        match self {
            Self::Spec => "Spec",
            Self::Jira => "Jira",
            Self::Dev => "Dev",
            Self::Pr => "PR",
        }
    }

    pub(super) fn state(self, record: &Record, jira: bool) -> StageState {
        match self {
            Self::Spec => match record.spec {
                SpecStatus::InProgress => StageState::Active,
                SpecStatus::Done => StageState::Done,
            },
            Self::Jira => match record.jira.status {
                JiraStatus::Waiting => StageState::Waiting,
                JiraStatus::Ready => StageState::Ready,
                JiraStatus::Created => StageState::Done,
            },
            Self::Dev => match record.implementation_stage(jira) {
                ImplementationStage::Locked => StageState::Locked,
                ImplementationStage::Ready => StageState::Ready,
                ImplementationStage::InProgress => StageState::Active,
                // Development is finished once its draft PR exists.
                ImplementationStage::DraftPr => StageState::Done,
            },
            Self::Pr => match record.pr_stage(jira) {
                PrStage::Locked => StageState::Locked,
                PrStage::Ready => StageState::Ready,
                PrStage::Draft => StageState::Draft,
            },
        }
    }

    pub(super) fn next(record: &Record, jira: bool) -> Self {
        if record.spec != SpecStatus::Done {
            Self::Spec
        } else if jira && record.jira.status != JiraStatus::Created {
            Self::Jira
        } else if matches!(
            record.implementation_stage(jira),
            ImplementationStage::Ready | ImplementationStage::Locked
        ) {
            Self::Dev
        } else {
            Self::Pr
        }
    }

    pub(super) fn actions(self, record: &Record, jira: bool) -> Vec<DetailAction> {
        match self {
            Self::Spec => {
                let mut actions = Vec::new();
                if record.spec == SpecStatus::InProgress {
                    actions.push(DetailAction::Finish);
                }
                actions.extend([DetailAction::ReadSpec, DetailAction::RefineSpec]);
                actions
            }
            Self::Jira => match record.jira.status {
                JiraStatus::Created => {
                    let mut actions = Vec::new();
                    if record.jira.url.is_some() {
                        actions.push(DetailAction::OpenJira);
                    }
                    actions.push(DetailAction::UpdateJira);
                    actions
                }
                JiraStatus::Ready => vec![DetailAction::Jira],
                JiraStatus::Waiting => Vec::new(),
            },
            Self::Dev => match record.implementation_stage(jira) {
                ImplementationStage::Ready => vec![DetailAction::Implement],
                ImplementationStage::InProgress | ImplementationStage::DraftPr => {
                    vec![DetailAction::UpdateImplementation]
                }
                ImplementationStage::Locked => Vec::new(),
            },
            Self::Pr => match record.pr_stage(jira) {
                PrStage::Draft => {
                    let mut actions = Vec::new();
                    if record.pr.url.is_some() {
                        actions.push(DetailAction::ReviewPr);
                    }
                    actions.push(DetailAction::UpdatePr);
                    actions
                }
                PrStage::Ready => vec![DetailAction::Pr],
                PrStage::Locked => Vec::new(),
            },
        }
    }

    pub(super) fn context(self, record: &Record, jira: bool) -> String {
        match self {
            Self::Spec => String::new(),
            Self::Jira => record.jira.key.clone().unwrap_or_else(|| {
                if record.jira.status == JiraStatus::Ready {
                    "Ticket needed"
                } else {
                    "After spec"
                }
                .into()
            }),
            Self::Dev => record
                .implementation
                .branch
                .clone()
                .or_else(|| record.implementation.agent.clone())
                .unwrap_or_else(|| {
                    match record.implementation_stage(jira) {
                        ImplementationStage::Ready => "Record agent and branch",
                        ImplementationStage::InProgress => "Implementation in progress",
                        ImplementationStage::DraftPr => "Implementation recorded",
                        ImplementationStage::Locked if jira => "Needs Jira",
                        ImplementationStage::Locked => "After spec",
                    }
                    .into()
                }),
            Self::Pr => record.pr.url.as_ref().map_or_else(
                || {
                    if record.pr_stage(jira) == PrStage::Ready {
                        "Ready to link"
                    } else {
                        "Needs implementation"
                    }
                    .into()
                },
                |url| {
                    format!(
                        "#{} ↗",
                        url.trim_end_matches('/').rsplit('/').next().unwrap_or(url)
                    )
                },
            ),
        }
    }

    pub(super) fn guidance(self, record: &Record, jira: bool) -> &'static str {
        match self {
            Self::Spec if record.spec == SpecStatus::Done => {
                "Your spec is ready. Read or refine it."
            }
            Self::Spec if jira => "Finish the spec to unlock Jira.",
            Self::Spec => "Finish the spec to unlock development.",
            Self::Jira if record.jira.status == JiraStatus::Created => {
                "Ticket linked. Open it or update the link."
            }
            Self::Jira if record.spec == SpecStatus::Done => {
                "Link a Jira ticket to unlock development."
            }
            Self::Jira => "Finish the spec before linking a ticket.",
            Self::Dev => match record.implementation_stage(jira) {
                ImplementationStage::Ready => "Record the implementer and branch.",
                ImplementationStage::Locked if jira => "Link Jira before starting development.",
                ImplementationStage::Locked => "Finish the spec before starting development.",
                ImplementationStage::InProgress | ImplementationStage::DraftPr => {
                    "Keep the implementer and branch up to date."
                }
            },
            Self::Pr => match record.pr_stage(jira) {
                PrStage::Draft => "Draft ready. Open it for review.",
                PrStage::Ready => "Link the draft PR when it is ready.",
                PrStage::Locked => "Start implementation before linking a PR.",
            },
        }
    }
}
