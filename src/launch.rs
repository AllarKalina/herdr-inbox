use crate::store::{Change, Launch, Record, Result, Store, absolute};
use serde_json::Value;
use std::env;
use std::path::PathBuf;
use std::process::Command;

pub const DEFAULT_WORKSPACE: &str = "AI herd";
pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
pub const DEFAULT_EFFORT: &str = "high";

pub struct Options {
    pub workspace: String,
    pub repo: Option<PathBuf>,
    pub model: String,
    pub effort: String,
    pub topic: String,
    pub bypass_permissions: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            workspace: DEFAULT_WORKSPACE.into(),
            repo: None,
            model: DEFAULT_MODEL.into(),
            effort: DEFAULT_EFFORT.into(),
            topic: String::new(),
            bypass_permissions: true,
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
            return Err(format!("Herdr {} failed: {}", args.join(" "), error.trim()).into());
        }
        Ok(serde_json::from_slice(&output.stdout)?)
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

fn prompt(record: &Record, topic: &str) -> Result<String> {
    let executable = env::current_exe()?;
    let lead = if topic.trim().is_empty() {
        "/grill-me".to_string()
    } else {
        format!("/grill-me {}", topic.trim())
    };
    Ok(format!(
        "{lead}\n\nThis session is inbox item {}. The title is intentionally unset until the spec is complete. Write the final Markdown spec to {}. When finished, choose a concise title and run: {} finish {} --title \"<title>\". Do not mark the spec done before the file is complete.",
        record.id,
        record.spec_path.display(),
        executable.display(),
        record.id,
    ))
}

fn mark(store: &Store, record: &Record, launch: &Launch) -> Result<Record> {
    store.update(&record.id, Change::Launch(launch.clone()))
}

pub fn start(store: &Store, mut options: Options) -> Result<Record> {
    let herdr = Herdr::new()?;
    let workspace_id = herdr.workspace(&options.workspace)?;
    if !executable_on_path("claude") {
        return Err("Claude Code is not installed or not on Herdr's PATH".into());
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

    let record = store.start_untitled(options.repo.clone())?;
    let mut launch = Launch {
        status: "starting".into(),
        workspace: options.workspace,
        workspace_id: Some(workspace_id.clone()),
        tab_id: None,
        pane_id: None,
        agent: None,
        model: options.model,
        effort: options.effort,
        prompt: String::new(),
        error: None,
    };
    launch.prompt = prompt(&record, &options.topic)?;
    mark(store, &record, &launch)?;
    let result = (|| -> Result<()> {
        let label = format!("Spec · {}", &record.id[..8]);
        let mut args = vec![
            "tab",
            "create",
            "--workspace",
            &workspace_id,
            "--label",
            &label,
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
        launch.status = "tab_opened".into();
        mark(store, &record, &launch)?;

        let agent = format!("spec_{}", &record.id[..8]);
        let pane = launch.pane_id.as_deref().ok_or("Missing pane ID")?;
        let mut args = vec![
            "agent",
            "start",
            &agent,
            "--kind",
            "claude",
            "--pane",
            pane,
            "--",
            "--model",
            &launch.model,
            "--effort",
            &launch.effort,
        ];
        if options.bypass_permissions {
            args.extend(["--permission-mode", "bypassPermissions"]);
        }
        herdr.call(&args)?;
        launch.agent = Some(agent.clone());
        launch.status = "agent_started".into();
        mark(store, &record, &launch)?;
        herdr.call(&["agent", "prompt", &agent, &launch.prompt])?;
        launch.status = "prompt_sent".into();
        mark(store, &record, &launch)?;
        herdr.call(&["workspace", "focus", &workspace_id])?;
        Ok(())
    })();
    if let Err(error) = result {
        launch.status = "failed".into();
        launch.error = Some(error.to_string());
        mark(store, &record, &launch)?;
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
}
