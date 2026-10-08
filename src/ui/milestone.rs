use super::{DetailAction, display_implementation_stage};
use crate::store::Record;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Milestone {
    Spec,
    Jira,
    Dev,
    Pr,
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

    pub(super) fn status(self, record: &Record, jira: bool) -> &str {
        match self {
            Self::Spec => &record.spec,
            Self::Jira => &record.jira.status,
            Self::Dev => display_implementation_stage(record, jira),
            Self::Pr => record.pr_stage(jira),
        }
    }

    pub(super) fn next(record: &Record, jira: bool) -> Self {
        if record.spec != "done" {
            Self::Spec
        } else if jira && record.jira.status != "created" {
            Self::Jira
        } else if record.implementation_stage(jira) == "ready"
            || record.implementation_stage(jira) == "locked"
        {
            Self::Dev
        } else {
            Self::Pr
        }
    }

    pub(super) fn actions(self, record: &Record, jira: bool) -> Vec<DetailAction> {
        match self {
            Self::Spec => {
                let mut actions = Vec::new();
                if record.spec == "in_progress" {
                    actions.push(DetailAction::Finish);
                }
                actions.extend([DetailAction::ReadSpec, DetailAction::RefineSpec]);
                actions
            }
            Self::Jira if record.jira.status == "created" => {
                let mut actions = Vec::new();
                if record.jira.url.is_some() {
                    actions.push(DetailAction::OpenJira);
                }
                actions.push(DetailAction::UpdateJira);
                actions
            }
            Self::Jira if record.jira.status == "ready" => vec![DetailAction::Jira],
            Self::Dev if record.implementation_stage(jira) == "ready" => {
                vec![DetailAction::Implement]
            }
            Self::Dev
                if matches!(
                    record.implementation.status.as_str(),
                    "in_progress" | "draft_pr"
                ) =>
            {
                vec![DetailAction::UpdateImplementation]
            }
            Self::Pr if record.pr.status == "draft" => {
                let mut actions = Vec::new();
                if record.pr.url.is_some() {
                    actions.push(DetailAction::ReviewPr);
                }
                actions.push(DetailAction::UpdatePr);
                actions
            }
            Self::Pr if record.pr_stage(jira) == "ready" => vec![DetailAction::Pr],
            _ => Vec::new(),
        }
    }

    pub(super) fn context(self, record: &Record, jira: bool) -> String {
        match self {
            Self::Spec => String::new(),
            Self::Jira => record.jira.key.clone().unwrap_or_else(|| {
                if record.jira.status == "ready" {
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
                        "ready" => "Record agent and branch",
                        "in_progress" => "Implementation in progress",
                        "draft_pr" => "Implementation recorded",
                        _ if jira => "Needs Jira",
                        _ => "After spec",
                    }
                    .into()
                }),
            Self::Pr => record.pr.url.as_ref().map_or_else(
                || {
                    if record.pr_stage(jira) == "ready" {
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
            Self::Spec if record.spec == "done" => "Your spec is ready. Read or refine it.",
            Self::Spec if jira => "Finish the spec to unlock Jira.",
            Self::Spec => "Finish the spec to unlock development.",
            Self::Jira if record.jira.status == "created" => {
                "Ticket linked. Open it or update the link."
            }
            Self::Jira if record.spec == "done" => "Link a Jira ticket to unlock development.",
            Self::Jira => "Finish the spec before linking a ticket.",
            Self::Dev if record.implementation_stage(jira) == "ready" => {
                "Record the implementer and branch."
            }
            Self::Dev if record.implementation_stage(jira) == "locked" && jira => {
                "Link Jira before starting development."
            }
            Self::Dev if record.implementation_stage(jira) == "locked" => {
                "Finish the spec before starting development."
            }
            Self::Dev => "Keep the implementer and branch up to date.",
            Self::Pr if record.pr.status == "draft" => "Draft ready. Open it for review.",
            Self::Pr if record.pr_stage(jira) == "ready" => "Link the draft PR when it is ready.",
            Self::Pr => "Start implementation before linking a PR.",
        }
    }
}
