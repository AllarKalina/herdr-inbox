//! The steps every session shares: open a tab, start the agent in it, hand over the prompt.
//! The caller hears about each completed step, so a failure shows how far the launch got.

use super::herdr::{Herdr, required_string};
use super::profile::{Profile, Session};
use crate::store::Result;
use std::path::Path;

/// One session to open.
pub(super) struct Plan<'a> {
    pub workspace_id: &'a str,
    /// The tab's label.
    pub label: &'a str,
    /// The agent's name inside Herdr; unique per session.
    pub agent: &'a str,
    pub profile: Profile,
    pub session: Session<'a>,
    /// The tab's working directory.
    pub repo: Option<&'a Path>,
    pub prompt: &'a str,
}

pub(super) enum Step {
    TabOpened { tab: String, pane: String },
    AgentStarted,
    PromptSent,
}

pub(super) fn open(
    herdr: &Herdr,
    plan: &Plan,
    mut completed: impl FnMut(Step) -> Result<()>,
) -> Result<()> {
    let mut args = vec!["tab", "create", "--workspace", plan.workspace_id];
    args.extend(["--label", plan.label, "--focus"]);
    let repo = plan.repo.map(|path| path.to_string_lossy().into_owned());
    if let Some(repo) = &repo {
        args.extend(["--cwd", repo]);
    }
    let tab = herdr.call(&args)?;
    let pane = required_string(&tab, &["result", "root_pane", "pane_id"])?.to_owned();
    completed(Step::TabOpened {
        tab: required_string(&tab, &["result", "tab", "tab_id"])?.into(),
        pane: pane.clone(),
    })?;

    let client = plan.profile.arguments(&plan.session);
    let kind = plan.profile.executable();
    let mut args = vec!["agent", "start", plan.agent, "--kind", kind];
    args.extend(["--pane", &pane, "--"]);
    args.extend(client.iter().map(String::as_str));
    herdr.start_agent(&args)?;
    completed(Step::AgentStarted)?;

    herdr.call(&["agent", "prompt", plan.agent, plan.prompt])?;
    completed(Step::PromptSent)
}
