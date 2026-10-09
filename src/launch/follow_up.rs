//! Sessions that follow a finished spec: an agent that creates its Jira ticket, and the
//! skill that implements it. The Inbox drives no ticketing or coding itself; it starts the
//! agent with instructions and waits for the agent to report back through the CLI.

use super::herdr::Herdr;
use super::profile::{DEFAULT_EFFORT, Profile, Session};
use super::session::{self, Plan};
use super::{WORKSPACE, prompts, readable_spec};
use crate::store::{Change, Record, Result, Store};

/// Follow-up work needs the tools and skills configured for Claude.
const PROFILE: Profile = Profile::Opus;

/// Asks an agent to create the item's Jira ticket, optionally under a parent epic or story.
/// The spec's own session is reused while it is still open, since that agent already knows
/// the spec; otherwise a new session starts.
pub fn ticket(store: &Store, id: &str, parent: Option<String>) -> Result<Record> {
    let record = store.get(id)?;
    // Check the rule before starting anything; the change itself is recorded afterwards.
    record.permits(Change::RequestJira, store.settings()?.jira)?;
    readable_spec(&record)?;
    let parent = parent
        .map(|key| key.trim().to_owned())
        .filter(|key| !key.is_empty());
    let herdr = Herdr::new()?;
    let prompt = prompts::ticket(&record, parent.as_deref(), store.path())?;
    match spec_agent(&herdr, &record) {
        Some(agent) => {
            herdr.call(&["agent", "prompt", agent, &prompt])?;
            // Focus is a convenience; the prompt has already been delivered.
            let _ = herdr.call(&["agent", "focus", agent]);
        }
        None => open(&herdr, store, &record, "Jira", "jira", &prompt)?,
    }
    store.update_settings(|settings| {
        if parent.is_some() {
            settings.jira_parent = parent;
        }
        Ok(())
    })?;
    store.update(id, Change::RequestJira)
}

/// Starts the configured development skill on the item and records that work has begun.
pub fn develop(store: &Store, id: &str) -> Result<Record> {
    let settings = store.settings()?;
    let skill = settings
        .dev_skill
        .as_deref()
        .ok_or("No development skill is configured; set one with settings defaults --dev-skill")?;
    let record = store.get(id)?;
    let started = || Change::Implement {
        agent: Some(skill.trim_start_matches(['/', '$']).to_owned()),
        branch: record.implementation.branch.clone(),
    };
    record.permits(started(), settings.jira)?;
    readable_spec(&record)?;
    let herdr = Herdr::new()?;
    let prompt = prompts::develop(&record, skill, store.path())?;
    open(&herdr, store, &record, "Dev", "dev", &prompt)?;
    store.update(id, started())
}

/// The spec session's agent, when it is a Claude session that is still running.
fn spec_agent<'a>(herdr: &Herdr, record: &'a Record) -> Option<&'a str> {
    let launch = record.launch.as_ref()?;
    let agent = launch.agent.as_deref()?;
    (launch.harness == PROFILE.id() && herdr.call(&["agent", "get", agent]).is_ok())
        .then_some(agent)
}

fn open(
    herdr: &Herdr,
    store: &Store,
    record: &Record,
    kind: &str,
    prefix: &str,
    prompt: &str,
) -> Result<()> {
    let workspace_id = herdr.workspace(WORKSPACE)?;
    let name = record
        .jira
        .key
        .clone()
        .unwrap_or_else(|| record.display_title().to_owned());
    let plan = Plan {
        workspace_id: &workspace_id,
        label: &format!("{kind} · {name}"),
        agent: &format!(
            "{prefix}_{}",
            &uuid::Uuid::new_v4().simple().to_string()[..12]
        ),
        profile: PROFILE,
        session: Session {
            model: PROFILE.default_model(),
            effort: DEFAULT_EFFORT,
            auto_permissions: true,
            data_dir: store.path(),
            spec_dir: record.spec_path.parent(),
        },
        prompt,
    };
    session::open(herdr, &plan, |_| Ok(()))?;
    // Focus is a convenience; the session is already running.
    let _ = herdr.call(&["workspace", "focus", &workspace_id]);
    Ok(())
}
