//! The Herdr command line, as far as the Inbox drives it.

use super::Result;
use serde_json::Value;
use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

pub struct Herdr {
    binary: PathBuf,
}

fn binary() -> PathBuf {
    env::var_os("HERDR_BIN_PATH").map_or_else(|| PathBuf::from("herdr"), PathBuf::from)
}

/// True inside a Herdr-managed pane, the only place tabs and agents can be driven from.
pub fn inside() -> bool {
    env::var("HERDR_ENV").as_deref() == Ok("1")
}

impl Herdr {
    pub fn new() -> Result<Self> {
        if !inside() {
            return Err("Start the inbox inside a Herdr-managed pane".into());
        }
        Ok(Self { binary: binary() })
    }

    pub fn call(&self, args: &[&str]) -> Result<Value> {
        let output = Command::new(&self.binary).args(args).output()?;
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            // A prompt is long; naming the operation is enough.
            let operation = if args.starts_with(&["agent", "prompt"]) {
                "agent prompt".into()
            } else {
                args.join(" ")
            };
            return Err(format!("Herdr {operation} failed: {}", error.trim()).into());
        }
        Ok(serde_json::from_slice(&output.stdout)?)
    }

    /// Starts an agent, waiting briefly while the new pane is still busy starting its shell.
    pub fn start_agent(&self, args: &[&str]) -> Result<Value> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match self.call(args) {
                Err(error)
                    if error.to_string().contains("agent_pane_busy")
                        && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(250));
                }
                result => return result,
            }
        }
    }

    pub fn workspace(&self, label: &str) -> Result<String> {
        workspace_id(&self.call(&["workspace", "list"])?, label)
    }
}

/// Opens the Inbox popup. Unlike the calls above, this also works from outside a pane.
pub fn open_inbox() -> Result<()> {
    let status = Command::new(binary())
        .args(["plugin", "pane", "open", "--plugin", "personal.inbox"])
        .args(["--entrypoint", "inbox"])
        .status()?;
    if !status.success() {
        return Err("Could not open Herdr inbox pane".into());
    }
    Ok(())
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

/// Reads a string the Herdr response must contain.
pub fn required_string<'a>(value: &'a Value, path: &[&str]) -> Result<&'a str> {
    let mut current = value;
    for key in path {
        current = &current[*key];
    }
    current
        .as_str()
        .ok_or_else(|| format!("Herdr response missing {}", path.join(".")).into())
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
