//! The agent clients a spec session can run in. Everything that differs between clients
//! lives here, so supporting another one means adding a variant and filling in its arms.

use super::Result;
use std::env;
use std::path::Path;

pub const DEFAULT_EFFORT: &str = "medium";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Opus,
    Codex,
}

impl Profile {
    pub const ALL: [Self; 2] = [Self::Opus, Self::Codex];

    pub fn parse(value: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|profile| profile.id() == value)
            .ok_or_else(|| format!("Unknown profile: {value} (choose opus or codex)").into())
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Opus => "opus",
            Self::Codex => "codex",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Opus => "Claude · Opus 5.5 · Medium",
            Self::Codex => "Codex · GPT-6.1-Sol · Medium",
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            Self::Opus => "claude-opus-5-5",
            Self::Codex => "gpt-6.1-sol",
        }
    }

    /// The client's executable, which is also the agent kind Herdr starts.
    pub fn executable(self) -> &'static str {
        match self {
            Self::Opus => "claude",
            Self::Codex => "codex",
        }
    }

    /// How this client invokes the grilling skill at the start of a prompt.
    pub fn skill(self) -> &'static str {
        match self {
            Self::Opus => "/grill-me",
            Self::Codex => "$grill-me",
        }
    }

    pub fn available(self) -> bool {
        env::var_os("PATH").is_some_and(|path| {
            env::split_paths(&path).any(|dir| dir.join(self.executable()).is_file())
        })
    }

    /// The client's own command-line arguments for one session.
    pub fn arguments(self, session: &Session) -> Vec<String> {
        let mut args = Vec::new();
        let mut push = |values: &[&str]| args.extend(values.iter().map(|value| value.to_string()));
        match self {
            Self::Opus => {
                push(&["--model", session.model, "--effort", session.effort]);
                if session.auto_permissions {
                    push(&["--permission-mode", "auto"]);
                }
            }
            Self::Codex => {
                let effort = format!("model_reasoning_effort=\"{}\"", session.effort);
                push(&["-m", session.model, "-c", &effort, "-s", "workspace-write"]);
                // The sandbox must reach the Inbox store to finish the item, and the spec.
                for directory in [Some(session.data_dir), session.spec_dir]
                    .into_iter()
                    .flatten()
                {
                    push(&["--add-dir", &directory.to_string_lossy()]);
                }
            }
        }
        args
    }
}

/// What a client needs to know to start one spec session.
pub struct Session<'a> {
    pub model: &'a str,
    pub effort: &'a str,
    pub auto_permissions: bool,
    pub data_dir: &'a Path,
    pub spec_dir: Option<&'a Path>,
}

pub fn available_profiles() -> Vec<Profile> {
    Profile::ALL
        .into_iter()
        .filter(|profile| profile.available())
        .collect()
}
