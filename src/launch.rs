//! Spec sessions: a Herdr tab running an agent client that interviews the user and writes
//! or refines one spec. The Inbox records how far the launch got, never what the agent did.

mod follow_up;
mod herdr;
mod profile;
mod prompts;
mod session;

pub use follow_up::{develop, ticket};
pub use herdr::open_inbox;
pub use profile::{Profile, available_profiles};

use crate::settings::Settings;
use crate::store::{Change, Launch, LaunchStatus, Record, Result, Store};
use herdr::Herdr;
use profile::Session;
use session::{Plan, Step};
use std::fs;
use uuid::Uuid;

/// Every session runs in this Herdr workspace, the folder agents are set up in.
pub const WORKSPACE: &str = "ai-boiler-room";

pub struct Options {
    pub profile: Profile,
    pub model: String,
    pub effort: String,
    pub topic: String,
    pub auto_permissions: bool,
}

impl Options {
    pub fn for_profile(profile: Profile) -> Self {
        Self {
            profile,
            model: profile.default_model().into(),
            effort: profile::DEFAULT_EFFORT.into(),
            topic: String::new(),
            auto_permissions: true,
        }
    }
}

/// Everything a session needs, checked before any record or tab is touched.
struct Ready {
    settings: Settings,
    herdr: Herdr,
    workspace_id: String,
}

fn preflight(store: &Store, options: &Options) -> Result<Ready> {
    let settings = store.settings()?;
    settings.validate_context()?;
    // Being inside Herdr comes first: without it nothing else about the session matters,
    // and the answer must not depend on what happens to be installed.
    let herdr = Herdr::new()?;
    if options.model.trim().is_empty() || options.effort.trim().is_empty() {
        return Err("Model and effort cannot be empty".into());
    }
    if !options.profile.available() {
        return Err(format!(
            "{} is not installed or not on Herdr's PATH",
            options.profile.executable()
        )
        .into());
    }
    let workspace_id = herdr.workspace(WORKSPACE)?;
    Ok(Ready {
        settings,
        herdr,
        workspace_id,
    })
}

fn new_launch(options: &Options, workspace_id: String, prompt: String) -> Launch {
    Launch {
        status: LaunchStatus::Starting,
        harness: options.profile.id().into(),
        workspace: WORKSPACE.into(),
        workspace_id: Some(workspace_id),
        tab_id: None,
        pane_id: None,
        agent: None,
        model: options.model.clone(),
        effort: options.effort.clone(),
        prompt,
        error: None,
    }
}

fn mark(store: &Store, record: &Record, launch: &Launch) -> Result<Record> {
    store.update(
        &record.id,
        Change::Launch(Box::new(launch.clone()), record.spec_path.clone()),
    )
}

/// What distinguishes one spec session from another.
struct Target {
    label: String,
    agent: String,
    refinement: bool,
}

/// Starts a session for a new spec. The session decides the spec's title, file name and
/// place inside the specs folder, and reports them when it finishes.
pub fn start(store: &Store, options: Options) -> Result<Record> {
    let ready = preflight(store, &options)?;
    let folder = ready
        .settings
        .sources
        .first()
        .ok_or("Choose a spec folder in settings before creating a spec")?
        .path
        .clone();
    let record = store.start_untitled()?;
    let prompt = prompts::initial(
        &record,
        &options.topic,
        options.profile,
        store.path(),
        &folder,
        &ready.settings.context_paths,
    )?;
    let mut launch = new_launch(&options, ready.workspace_id, prompt);
    mark(store, &record, &launch)?;
    let target = Target {
        label: format!("Spec · {}", &record.id[..8]),
        agent: format!("spec_{}", &record.id[..8]),
        refinement: false,
    };
    let result = run(store, &ready.herdr, &record, &mut launch, &options, &target);
    // Without a tab there is no session to inspect: leave the Inbox as it was.
    if result.is_err() && launch.tab_id.is_none() {
        store.discard_unstarted(&record.id)?;
    }
    result
}

/// Starts another session on an existing item and its existing spec file.
pub fn refine(store: &Store, id: &str, options: Options) -> Result<Record> {
    let record = store.get(id)?;
    if record.active_spec_session() {
        return Err(
            "Finish or settle the current spec session before starting another refinement".into(),
        );
    }
    readable_spec(&record)?;
    let ready = preflight(store, &options)?;
    let prompt = prompts::refinement(
        &record,
        &options.topic,
        options.profile,
        store.path(),
        &ready.settings.context_paths,
    )?;
    let mut launch = new_launch(&options, ready.workspace_id, prompt);
    store.update(
        id,
        Change::BeginRefinement(Box::new(launch.clone()), record.spec_path.clone()),
    )?;
    // Repeated refinements of one item each need their own tab and agent name.
    let session_id = Uuid::new_v4().simple().to_string();
    let target = Target {
        label: format!("Refine · {} · {}", record.display_title(), &session_id[..8]),
        agent: format!("refine_{}", &session_id[..24]),
        refinement: true,
    };
    run(store, &ready.herdr, &record, &mut launch, &options, &target)
}

/// Opens a spec session, recording each completed step on the item, and a failure for
/// later inspection.
fn run(
    store: &Store,
    herdr: &Herdr,
    record: &Record,
    launch: &mut Launch,
    options: &Options,
    target: &Target,
) -> Result<Record> {
    let workspace_id = launch.workspace_id.clone().ok_or("Missing workspace ID")?;
    let (model, effort, prompt) = (
        launch.model.clone(),
        launch.effort.clone(),
        launch.prompt.clone(),
    );
    let plan = Plan {
        workspace_id: &workspace_id,
        label: &target.label,
        agent: &target.agent,
        profile: options.profile,
        session: Session {
            model: &model,
            effort: &effort,
            auto_permissions: options.auto_permissions,
            data_dir: store.path(),
            spec_dir: record.spec_path.parent(),
        },
        prompt: &prompt,
    };
    let opened = session::open(herdr, &plan, |step| {
        match step {
            Step::TabOpened { tab, pane } => {
                launch.tab_id = Some(tab);
                launch.pane_id = Some(pane);
                launch.status = LaunchStatus::TabOpened;
            }
            Step::AgentStarted => {
                launch.agent = Some(target.agent.clone());
                launch.status = LaunchStatus::AgentStarted;
            }
            Step::PromptSent => {
                if target.refinement {
                    // The spec is open again only once an agent has accepted the work.
                    store.update(&record.id, Change::RefineSpec)?;
                }
                launch.status = LaunchStatus::PromptSent;
            }
        }
        mark(store, record, launch).map(|_| ())
    });
    if let Err(error) = opened {
        launch.status = LaunchStatus::Failed;
        launch.error = Some(error.to_string());
        mark(store, record, launch)?;
        return Err(error);
    }
    if let Err(error) = herdr.call(&["workspace", "focus", &workspace_id]) {
        launch.error = Some(format!(
            "Session started, but workspace focus failed: {error}. Select the new tab manually."
        ));
        mark(store, record, launch)?;
    }
    store.get(&record.id)
}

fn readable_spec(record: &Record) -> Result<()> {
    if !record.spec_path.is_file() {
        return Err(format!("Spec file is missing: {}", record.spec_path.display()).into());
    }
    let text = fs::read_to_string(&record.spec_path).map_err(|error| {
        format!(
            "Cannot read spec file {}: {error}",
            record.spec_path.display()
        )
    })?;
    if text.trim().is_empty() {
        return Err(format!("Spec file is empty: {}", record.spec_path.display()).into());
    }
    Ok(())
}

/// Names the session's tab after the spec. Outside Herdr there is no tab to rename.
pub fn rename_tab(record: &Record) -> Result<()> {
    let tab = record
        .launch
        .as_ref()
        .and_then(|launch| launch.tab_id.as_deref());
    if let Some(tab) = tab
        && herdr::inside()
    {
        Herdr::new()?.call(&["tab", "rename", tab, record.display_title()])?;
    }
    Ok(())
}
