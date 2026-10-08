use super::*;

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
    let label = target.label.as_str();
    let agent = target.agent.as_str();
    let workspace_id = launch.workspace_id.clone().ok_or("Missing workspace ID")?;
    let workspace_id = workspace_id.as_str();
    let mut args = vec![
        "tab",
        "create",
        "--workspace",
        workspace_id,
        "--label",
        label,
        "--focus",
    ];
    let repo_string = options
        .repo
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());
    if let Some(repo) = &repo_string {
        args.extend(["--cwd", repo]);
    }
    let tab = herdr.call(&args)?;
    launch.tab_id = Some(required_string(&tab, &["result", "tab", "tab_id"])?.into());
    launch.pane_id = Some(required_string(&tab, &["result", "root_pane", "pane_id"])?.into());
    launch.status = LaunchStatus::TabOpened;
    mark(store, record, launch)?;

    let pane = launch.pane_id.as_deref().ok_or("Missing pane ID")?;
    let mut args = vec![
        "agent",
        "start",
        agent,
        "--kind",
        options.profile.kind(),
        "--pane",
        pane,
        "--",
    ];
    let effort_config = format!("model_reasoning_effort=\"{}\"", launch.effort);
    let data_dir = store.path().to_string_lossy().to_string();
    let spec_dir = record
        .spec_path
        .parent()
        .map(|path| path.to_string_lossy().to_string());
    match options.profile {
        Profile::Opus => {
            args.extend(["--model", &launch.model, "--effort", &launch.effort]);
            if options.bypass_permissions {
                args.extend(["--permission-mode", "bypassPermissions"]);
            }
        }
        Profile::Codex => {
            args.extend([
                "-m",
                &launch.model,
                "-c",
                &effort_config,
                "-s",
                "workspace-write",
                "--add-dir",
                &data_dir,
            ]);
            if let Some(directory) = &spec_dir {
                args.extend(["--add-dir", directory]);
            }
        }
    }
    herdr.start_agent(&args)?;
    launch.agent = Some(agent.to_string());
    launch.status = LaunchStatus::AgentStarted;
    mark(store, record, launch)?;
    herdr.call(&["agent", "prompt", agent, &launch.prompt])?;
    if target.refinement {
        store.update(&record.id, Change::RefineSpec)?;
    }
    launch.status = LaunchStatus::PromptSent;
    mark(store, record, launch)?;
    if let Err(error) = herdr.call(&["workspace", "focus", workspace_id]) {
        launch.error = Some(format!(
            "Session started, but workspace focus failed: {error}. Select the new tab manually."
        ));
        mark(store, record, launch)?;
    }
    Ok(())
}
