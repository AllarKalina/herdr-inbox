use crate::store::{Change, Launch, LaunchStatus, Record, Result, Store, absolute};
use serde_json::Value;
use std::env;
use std::fs;
use uuid::Uuid;

mod prompts;
mod session;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

pub const DEFAULT_WORKSPACE: &str = "ai-boiler-room";
pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
pub const DEFAULT_EFFORT: &str = "high";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Opus,
    Codex,
}

impl Profile {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "opus" => Ok(Self::Opus),
            "codex" => Ok(Self::Codex),
            _ => Err(format!("Unknown profile: {value} (choose opus or codex)").into()),
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Opus => "opus",
            Self::Codex => "codex",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Opus => "Claude · Opus 5.5 · High",
            Self::Codex => "Codex · GPT-6.1-Sol · High",
        }
    }

    fn executable(self) -> &'static str {
        match self {
            Self::Opus => "claude",
            Self::Codex => "codex",
        }
    }

    fn kind(self) -> &'static str {
        self.executable()
    }

    fn skill(self) -> &'static str {
        match self {
            Self::Opus => "/grill-me",
            Self::Codex => "$grill-me",
        }
    }

    pub fn available(self) -> bool {
        executable_on_path(self.executable())
    }
}

pub fn available_profiles() -> Vec<Profile> {
    [Profile::Opus, Profile::Codex]
        .into_iter()
        .filter(|profile| profile.available())
        .collect()
}

pub struct Options {
    pub profile: Profile,
    pub workspace: String,
    pub repo: Option<PathBuf>,
    pub spec: Option<PathBuf>,
    pub model: String,
    pub effort: String,
    pub topic: String,
    pub bypass_permissions: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            profile: Profile::Opus,
            workspace: DEFAULT_WORKSPACE.into(),
            repo: None,
            spec: None,
            model: DEFAULT_MODEL.into(),
            effort: DEFAULT_EFFORT.into(),
            topic: String::new(),
            bypass_permissions: true,
        }
    }
}

impl Options {
    pub fn for_profile(profile: Profile) -> Self {
        Self {
            profile,
            model: match profile {
                Profile::Opus => DEFAULT_MODEL,
                Profile::Codex => "gpt-6.1-sol",
            }
            .into(),
            ..Self::default()
        }
    }
}

struct Herdr {
    binary: PathBuf,
}

impl Herdr {
    fn new() -> Result<Self> {
        if env::var("HERDR_ENV").as_deref() != Ok("1") {
            return Err("Start the inbox inside a Herdr-managed pane".into());
        }
        Ok(Self {
            binary: env::var_os("HERDR_BIN_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("herdr")),
        })
    }

    fn call(&self, args: &[&str]) -> Result<Value> {
        let output = Command::new(&self.binary).args(args).output()?;
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            let operation = if args.starts_with(&["agent", "prompt"]) {
                "agent prompt".into()
            } else {
                args.join(" ")
            };
            return Err(format!("Herdr {operation} failed: {}", error.trim()).into());
        }
        Ok(serde_json::from_slice(&output.stdout)?)
    }

    fn start_agent(&self, args: &[&str]) -> Result<Value> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match self.call(args) {
                Ok(value) => return Ok(value),
                Err(error)
                    if error.to_string().contains("agent_pane_busy")
                        && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(250));
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn workspace(&self, label: &str) -> Result<String> {
        let response = self.call(&["workspace", "list"])?;
        workspace_id(&response, label)
    }
}

fn workspace_id(response: &Value, label: &str) -> Result<String> {
    let workspaces = response["result"]["workspaces"]
        .as_array()
        .ok_or("Herdr workspace list has no workspaces")?;
    let matches: Vec<&Value> = workspaces
        .iter()
        .filter(|workspace| workspace["label"].as_str() == Some(label))
        .collect();
    if matches.len() != 1 {
        let available = workspaces
            .iter()
            .filter_map(|workspace| workspace["label"].as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let reason = if matches.is_empty() {
            "not found"
        } else {
            "ambiguous"
        };
        return Err(format!("Workspace '{label}' {reason}. Available: {available}").into());
    }
    Ok(matches[0]["workspace_id"]
        .as_str()
        .ok_or("Workspace has no ID")?
        .into())
}

fn executable_on_path(name: &str) -> bool {
    env::var_os("PATH")
        .is_some_and(|path| env::split_paths(&path).any(|dir| dir.join(name).is_file()))
}

fn required_string<'a>(value: &'a Value, path: &[&str]) -> Result<&'a str> {
    let mut current = value;
    for key in path {
        current = &current[*key];
    }
    current
        .as_str()
        .ok_or_else(|| format!("Herdr response missing {}", path.join(".")).into())
}

fn mark(store: &Store, record: &Record, launch: &Launch) -> Result<Record> {
    store.update(
        &record.id,
        Change::Launch(Box::new(launch.clone()), record.spec_path.clone()),
    )
}

pub fn start(store: &Store, mut options: Options) -> Result<Record> {
    let settings = store.settings()?;
    settings.validate_context()?;
    let herdr = Herdr::new()?;
    let workspace_id = herdr.workspace(&options.workspace)?;
    if !options.profile.available() {
        return Err(format!(
            "{} is not installed or not on Herdr's PATH",
            options.profile.executable()
        )
        .into());
    }
    options.repo = options.repo.map(absolute).transpose()?;
    if let Some(repo) = &options.repo
        && !repo.is_dir()
    {
        return Err(format!("Repo directory does not exist: {}", repo.display()).into());
    }
    if options.model.trim().is_empty() || options.effort.trim().is_empty() {
        return Err("Model and effort cannot be empty".into());
    }

    let record = match options.spec.clone() {
        Some(path) => store.start_untitled_with_spec(options.repo.clone(), Some(path))?,
        None => store.start_untitled(options.repo.clone())?,
    };
    let mut launch = Launch {
        status: LaunchStatus::Starting,
        harness: options.profile.id().into(),
        workspace: options.workspace.clone(),
        workspace_id: Some(workspace_id.clone()),
        tab_id: None,
        pane_id: None,
        agent: None,
        model: options.model.clone(),
        effort: options.effort.clone(),
        prompt: String::new(),
        error: None,
    };
    launch.prompt = prompts::initial(
        &record,
        &options.topic,
        options.profile,
        store.path(),
        &settings.context_paths,
    )?;
    mark(store, &record, &launch)?;
    let result = complete(
        store,
        &herdr,
        &record,
        &mut launch,
        &options,
        session::Target {
            label: format!("Spec · {}", &record.id[..8]),
            agent: format!("spec_{}", &record.id[..8]),
            refinement: false,
        },
    );
    // Without a tab there is no session to inspect: leave the Inbox as it was.
    if result.is_err() && launch.tab_id.is_none() {
        store.discard_unstarted(&record.id)?;
    }
    result
}

pub fn refine(store: &Store, id: &str, mut options: Options) -> Result<Record> {
    let settings = store.settings()?;
    settings.validate_context()?;
    let record = store.get(id)?;
    if record.active_spec_session() {
        return Err(
            "Finish or settle the current spec session before starting another refinement".into(),
        );
    }
    if options.spec.is_some() {
        return Err(
            "Refinement uses the existing spec path; use relink before starting a session".into(),
        );
    }
    preflight_spec(&record)?;
    options.repo = options
        .repo
        .or_else(|| record.repo.clone())
        .map(absolute)
        .transpose()?;
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
    let mut launch = Launch {
        status: LaunchStatus::Starting,
        harness: options.profile.id().into(),
        workspace: options.workspace.clone(),
        workspace_id: Some(workspace_id),
        tab_id: None,
        pane_id: None,
        agent: None,
        model: options.model.clone(),
        effort: options.effort.clone(),
        prompt: prompts::refinement(
            &record,
            &options.topic,
            options.profile,
            store.path(),
            options.repo.as_deref(),
            &settings.context_paths,
        )?,
        error: None,
    };
    let session_id = Uuid::new_v4().simple().to_string();
    store.update(
        id,
        Change::BeginRefinement(Box::new(launch.clone()), record.spec_path.clone()),
    )?;
    complete(
        store,
        &herdr,
        &record,
        &mut launch,
        &options,
        session::Target {
            label: format!("Refine · {} · {}", record.display_title(), &session_id[..8]),
            agent: format!("refine_{}", &session_id[..24]),
            refinement: true,
        },
    )
}

fn preflight_spec(record: &Record) -> Result<()> {
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

fn complete(
    store: &Store,
    herdr: &Herdr,
    record: &Record,
    launch: &mut Launch,
    options: &Options,
    target: session::Target,
) -> Result<Record> {
    if let Err(error) = session::run(store, herdr, record, launch, options, &target) {
        launch.status = LaunchStatus::Failed;
        launch.error = Some(error.to_string());
        mark(store, record, launch)?;
        return Err(error);
    }
    store.get(&record.id)
}

pub fn rename_tab(record: &Record) -> Result<()> {
    if env::var("HERDR_ENV").as_deref() != Ok("1") {
        return Ok(());
    }
    if let Some(tab) = record
        .launch
        .as_ref()
        .and_then(|launch| launch.tab_id.as_deref())
    {
        let herdr = Herdr::new()?;
        herdr.call(&["tab", "rename", tab, record.display_title()])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_exact_workspace_label() -> Result<()> {
        let response = serde_json::json!({"result":{"workspaces":[
            {"label":"main","workspace_id":"w1"},
            {"label":"AI herd","workspace_id":"w2"}
        ]}});
        assert_eq!(workspace_id(&response, "AI herd")?, "w2");
        assert!(workspace_id(&response, "ai herd").is_err());
        Ok(())
    }

    #[test]
    fn refinement_missing_or_blank_spec_leaves_record_unchanged() -> Result<()> {
        let root = env::temp_dir().join(format!("herdr-refine-preflight-{}", Uuid::new_v4()));
        let store = Store::new(root.clone());
        let source = root.join("specs");
        fs::create_dir_all(&source)?;
        let mut settings = store.settings()?;
        settings
            .sources
            .push(crate::store::SpecSource::new(source)?);
        store.save_settings(&settings)?;
        let record = store.start("Existing", None, None)?;
        store.update(&record.id, Change::Finish { title: None })?;
        let item = root.join("items").join(format!("{}.json", record.id));
        let before = fs::read(&item)?;
        fs::remove_file(&record.spec_path)?;
        let error = refine(&store, &record.id, Options::default()).unwrap_err();
        assert!(error.to_string().contains("Spec file is missing"));
        assert_eq!(fs::read(&item)?, before);
        fs::write(&record.spec_path, " \n\t")?;
        let error = refine(&store, &record.id, Options::default()).unwrap_err();
        assert!(error.to_string().contains("Spec file is empty"));
        assert_eq!(fs::read(&item)?, before);
        fs::remove_dir_all(root)?;
        Ok(())
    }
}
