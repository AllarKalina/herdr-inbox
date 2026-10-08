use super::{App, Milestone, Prompt, Screen, nonempty, settings};
use crate::launch::{self, Options};
use crate::store::{Change, Result};
use std::path::PathBuf;

impl App {
    pub(super) fn submit(&mut self) -> Result<()> {
        let Some(prompt) = self.prompt.take() else {
            return Ok(());
        };
        let value = std::mem::take(&mut self.input).trim().to_string();
        match prompt {
            Prompt::LaunchWorkspace { profile } => self.begin(Prompt::LaunchRepo {
                profile,
                workspace: if value.is_empty() {
                    self.store.settings()?.workspace
                } else {
                    value
                },
            }),
            Prompt::LaunchRepo { profile, workspace } => self.begin(Prompt::LaunchSpec {
                profile,
                workspace,
                repo: nonempty(value).map(PathBuf::from),
            }),
            Prompt::LaunchSpec {
                profile,
                workspace,
                repo,
            } => self.begin(Prompt::LaunchTopic {
                profile,
                workspace,
                repo,
                spec: if value.is_empty() {
                    None
                } else {
                    Some(settings::path(&value)?)
                },
            }),
            Prompt::Settle { id } => {
                self.store.settle(&id)?;
                self.message = "Session settled; its spec and progress stay unchanged".into();
            }
            Prompt::Relink { id } => {
                let record = self.store.relink(&id, settings::path(&value)?)?;
                self.message = format!("Relinked {}", record.display_title());
            }
            Prompt::LaunchTopic {
                profile,
                workspace,
                repo,
                spec,
            } => {
                let options = Options {
                    workspace,
                    repo,
                    spec,
                    topic: value,
                    ..Options::for_profile(profile)
                };
                let record = launch::start(&self.store, options)?;
                self.message = format!("Launched spec session {}", record.id);
                self.should_exit = true;
            }
            Prompt::FinishTitle { id } if !value.is_empty() => {
                let record = self
                    .store
                    .update(&id, Change::Finish { title: Some(value) })?;
                let _ = launch::rename_tab(&record);
                self.acknowledge(Milestone::Spec);
            }
            Prompt::Jira { id } if !value.is_empty() => {
                let url = self.store.get(&id)?.jira.url.unwrap_or_default();
                self.begin(Prompt::JiraUrl { id, key: value });
                self.input = url;
            }
            Prompt::JiraUrl { id, key } => {
                self.store.update(
                    &id,
                    Change::Jira {
                        key,
                        url: nonempty(value),
                    },
                )?;
                self.acknowledge(Milestone::Jira);
            }
            Prompt::Agent { id } => {
                let branch = self
                    .store
                    .get(&id)?
                    .implementation
                    .branch
                    .unwrap_or_default();
                self.begin(Prompt::Branch {
                    id,
                    agent: nonempty(value),
                });
                self.input = branch;
            }
            Prompt::Branch { id, agent } => {
                self.store.update(
                    &id,
                    Change::Implement {
                        agent,
                        branch: nonempty(value),
                    },
                )?;
                self.acknowledge(Milestone::Dev);
            }
            Prompt::Pr { id } if !value.is_empty() => {
                self.store.update(&id, Change::Pr { url: value })?;
                self.acknowledge(Milestone::Pr);
            }
            Prompt::Delete { id } => {
                self.store.delete(&id)?;
                self.message = "Item moved to local Trash".into();
                self.screen = Screen::List;
            }
            _ => self.message = "Cancelled".into(),
        }
        self.refresh()?;
        Ok(())
    }
}
