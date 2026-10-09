//! Spec sessions: a Herdr tab running an agent client that interviews the user and writes
//! or refines one spec. The Inbox records how far the launch got, never what the agent did.

mod herdr;
mod profile;
mod prompts;
mod session;

pub use herdr::open_inbox;
pub use profile::{Profile, available_profiles};

use crate::settings::{DEFAULT_WORKSPACE, Settings};
use crate::store::{Change, Launch, LaunchStatus, Record, Result, Store, absolute};
use herdr::Herdr;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

pub struct Options {
    pub profile: Profile,
    pub workspace: String,
    pub repo: Option<PathBuf>,
    pub model: String,
    pub effort: String,
    pub topic: String,
    pub bypass_permissions: bool,
}

impl Options {
    pub fn for_profile(profile: Profile) -> Self {
        Self {
            profile,
            workspace: DEFAULT_WORKSPACE.into(),
            repo: None,
            model: profile.default_model().into(),
            effort: profile::DEFAULT_EFFORT.into(),
            topic: String::new(),
            bypass_permissions: true,
        }
    }
}

/// Everything a session needs, checked before any record or tab is touched.
struct Ready {
    settings: Settings,
    herdr: Herdr,
    workspace_id: String,
}

fn preflight(store: &Store, options: &mut Options) -> Result<Ready> {
    let settings = store.settings()?;
    settings.validate_context()?;
    options.repo = options.repo.take().map(absolute).transpose()?;
    if let Some(repo) = &options.repo
        && !repo.is_dir()
    {
        return Err(format!("Repo directory does not exist: {}", repo.display()).into());
    }
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
    let herdr = Herdr::new()?;
    let workspace_id = herdr.workspace(&options.workspace)?;
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
        workspace: options.workspace.clone(),
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

/// Starts a session for a new, untitled spec. `spec` names its file; without one the file
/// gets a generated name in the first selected folder.
pub fn start(store: &Store, mut options: Options, spec: Option<PathBuf>) -> Result<Record> {
    let ready = preflight(store, &mut options)?;
    let record = store.start_untitled(options.repo.clone(), spec)?;
    let prompt = prompts::initial(
        &record,
        &options.topic,
        options.profile,
        store.path(),
        &ready.settings.context_paths,
    )?;
    let mut launch = new_launch(&options, ready.workspace_id, prompt);
    mark(store, &record, &launch)?;
    let target = session::Target {
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
pub fn refine(store: &Store, id: &str, mut options: Options) -> Result<Record> {
    let record = store.get(id)?;
    if record.active_spec_session() {
        return Err(
            "Finish or settle the current spec session before starting another refinement".into(),
        );
    }
    readable_spec(&record)?;
    options.repo = options.repo.take().or_else(|| record.repo.clone());
    let ready = preflight(store, &mut options)?;
    let prompt = prompts::refinement(
        &record,
        &options.topic,
        options.profile,
        store.path(),
        options.repo.as_deref(),
        &ready.settings.context_paths,
    )?;
    let mut launch = new_launch(&options, ready.workspace_id, prompt);
    store.update(
        id,
        Change::BeginRefinement(Box::new(launch.clone()), record.spec_path.clone()),
    )?;
    // Repeated refinements of one item each need their own tab and agent name.
    let session_id = Uuid::new_v4().simple().to_string();
    let target = session::Target {
        label: format!("Refine · {} · {}", record.display_title(), &session_id[..8]),
        agent: format!("refine_{}", &session_id[..24]),
        refinement: true,
    };
    run(store, &ready.herdr, &record, &mut launch, &options, &target)
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

/// Runs the session steps and records a failure on the item for later inspection.
fn run(
    store: &Store,
    herdr: &Herdr,
    record: &Record,
    launch: &mut Launch,
    options: &Options,
    target: &session::Target,
) -> Result<Record> {
    if let Err(error) = session::run(store, herdr, record, launch, options, target) {
        launch.status = LaunchStatus::Failed;
        launch.error = Some(error.to_string());
        mark(store, record, launch)?;
        return Err(error);
    }
    store.get(&record.id)
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
