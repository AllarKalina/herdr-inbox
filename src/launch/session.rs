//! The steps of one launch: open a tab, start the agent in it, hand over the prompt.
//! Each completed step is recorded, so a failure shows exactly how far the launch got.

use super::herdr::required_string;
use super::profile::Session;
use super::{Herdr, Launch, LaunchStatus, Options, Record, Result, Store, mark};
use crate::store::Change;

pub(super) struct Target {
    pub label: String,
    pub agent: String,
    pub refinement: bool,
}

pub(super) fn run(
    store: &Store,
    herdr: &Herdr,
    record: &Record,
    launch: &mut Launch,
    options: &Options,
    target: &Target,
) -> Result<()> {
    let workspace = launch.workspace_id.clone().ok_or("Missing workspace ID")?;
    let mut args = vec!["tab", "create", "--workspace", &workspace];
    args.extend(["--label", &target.label, "--focus"]);
    let repo = options
        .repo
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned());
    if let Some(repo) = &repo {
        args.extend(["--cwd", repo]);
    }
    let tab = herdr.call(&args)?;
    let pane = required_string(&tab, &["result", "root_pane", "pane_id"])?.to_owned();
    launch.tab_id = Some(required_string(&tab, &["result", "tab", "tab_id"])?.into());
    launch.pane_id = Some(pane.clone());
    launch.status = LaunchStatus::TabOpened;
    mark(store, record, launch)?;

    let client = options.profile.arguments(&Session {
        model: &launch.model,
        effort: &launch.effort,
        bypass_permissions: options.bypass_permissions,
        data_dir: store.path(),
        spec_dir: record.spec_path.parent(),
    });
    let kind = options.profile.executable();
    let mut args = vec!["agent", "start", &target.agent, "--kind", kind];
    args.extend(["--pane", &pane, "--"]);
    args.extend(client.iter().map(String::as_str));
    herdr.start_agent(&args)?;
    launch.agent = Some(target.agent.clone());
    launch.status = LaunchStatus::AgentStarted;
    mark(store, record, launch)?;

    herdr.call(&["agent", "prompt", &target.agent, &launch.prompt])?;
    if target.refinement {
        // The spec is open again only once an agent has actually accepted the work.
        store.update(&record.id, Change::RefineSpec)?;
    }
    launch.status = LaunchStatus::PromptSent;
    mark(store, record, launch)?;

    if let Err(error) = herdr.call(&["workspace", "focus", &workspace]) {
        launch.error = Some(format!(
            "Session started, but workspace focus failed: {error}. Select the new tab manually."
        ));
        mark(store, record, launch)?;
    }
    Ok(())
}
