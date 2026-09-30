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

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Spec => "SPEC",
            Self::Jira => "JIRA",
            Self::Dev => "DEV",
            Self::Pr => "PR",
        }
    }

    pub(super) fn status(self, record: &Record) -> &str {
        match self {
            Self::Spec => &record.spec,
            Self::Jira => &record.jira.status,
            Self::Dev => display_implementation_stage(record),
            Self::Pr => record.pr_stage(),
        }
    }

    pub(super) fn next(record: &Record) -> Self {
        if record.spec != "done" {
            Self::Spec
        } else if record.jira.status != "created" {
            Self::Jira
        } else if record.implementation_stage() == "ready"
            || record.implementation_stage() == "locked"
        {
            Self::Dev
        } else {
            Self::Pr
        }
    }

    pub(super) fn actions(self, record: &Record) -> Vec<DetailAction> {
        match self {
            Self::Spec => {
                let mut actions = Vec::new();
                if record.spec == "in_progress" {
                    actions.push(DetailAction::Finish);
                }
                actions.extend([DetailAction::ReadSpec, DetailAction::EditSpec]);
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
            Self::Dev if record.implementation_stage() == "ready" => {
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
            Self::Pr if record.pr_stage() == "ready" => vec![DetailAction::Pr],
            _ => Vec::new(),
        }
    }

    pub(super) fn context(self, record: &Record) -> String {
        match self {
            Self::Spec => record.launch.as_ref().map_or_else(
                || {
                    if record.spec == "done" {
                        "Spec complete"
                    } else {
                        "Writing spec"
                    }
                    .into()
                },
                |launch| {
                    if launch.status == "failed" {
                        "Launch failed".into()
                    } else {
                        format!("{} · {}", launch.workspace, launch.harness)
                    }
                },
            ),
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
                    match record.implementation_stage() {
                        "ready" => "Ready to start",
                        "in_progress" => "Implementation in progress",
                        "draft_pr" => "Implementation recorded",
                        _ => "Needs Jira",
                    }
                    .into()
                }),
            Self::Pr => record.pr.url.as_ref().map_or_else(
                || {
                    if record.pr_stage() == "ready" {
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

    pub(super) fn guidance(self, record: &Record) -> &'static str {
        match self {
            Self::Spec if record.spec == "done" => "Your spec is ready. Read or refine it.",
            Self::Spec => "Finish the spec to unlock Jira.",
            Self::Jira if record.jira.status == "created" => {
                "Ticket linked. Open it or update the link."
            }
            Self::Jira if record.spec == "done" => "Link a Jira ticket to unlock development.",
            Self::Jira => "Finish the spec before linking a ticket.",
            Self::Dev if record.implementation_stage() == "ready" => {
                "Record the implementer and branch."
            }
            Self::Dev if record.implementation_stage() == "locked" => {
                "Link Jira before starting development."
            }
            Self::Dev => "Keep the implementer and branch up to date.",
            Self::Pr if record.pr.status == "draft" => "Draft ready. Open it for review.",
            Self::Pr if record.pr_stage() == "ready" => "Link the draft PR when it is ready.",
            Self::Pr => "Start implementation before linking a PR.",
        }
    }
}
