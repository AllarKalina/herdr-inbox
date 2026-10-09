//! The words that start a session. The agent learns which item and file it owns and the
//! exact command that reports completion back to the Inbox.

use super::{Profile, Record, Result};
use std::env;
use std::path::{Path, PathBuf};

pub(super) fn initial(
    record: &Record,
    topic: &str,
    profile: Profile,
    data_dir: &Path,
    context_paths: &[PathBuf],
) -> Result<String> {
    let executable = env::current_exe()?;
    let context = context_references(context_paths);
    let lead = if topic.trim().is_empty() {
        profile.skill().to_string()
    } else {
        format!("{} {}", profile.skill(), topic.trim())
    };
    Ok(format!(
        "{lead}\n{context}\nThis session is inbox item {}. The title is intentionally unset until the spec is complete. Write the final Markdown spec to {}. When finished, choose a concise title and run: HERDR_INBOX_HOME={} {} finish {} --title \"<title>\". Do not mark the spec done before the file is complete.",
        record.id,
        shell_quote(&record.spec_path.to_string_lossy()),
        shell_quote(&data_dir.to_string_lossy()),
        shell_quote(&executable.to_string_lossy()),
        shell_quote(&record.id),
    ))
}

pub(super) fn refinement(
    record: &Record,
    topic: &str,
    profile: Profile,
    data_dir: &Path,
    repo: Option<&Path>,
    context_paths: &[PathBuf],
) -> Result<String> {
    let context = context_references(context_paths);
    let executable = shell_quote(&env::current_exe()?.to_string_lossy());
    let spec = shell_quote(&record.spec_path.to_string_lossy());
    let id = shell_quote(&record.id);
    let inbox = shell_quote(&data_dir.to_string_lossy());
    let repo = match repo {
        Some(path) => format!(
            "The launch repository directory is {}. Begin inspecting there, and identify any other affected repositories and services from the spec, instructions, or paths the user provides. Do not guess repository paths.",
            shell_quote(&path.to_string_lossy())
        ),
        None => String::new(),
    };
    let topic = if topic.trim().is_empty() {
        String::new()
    } else {
        format!("\nInitial context from the user: {}", topic.trim())
    };
    Ok(format!(
        "{}\n\nRefine the existing spec for inbox item {} ({}) at {spec}. Reuse this item and this exact Markdown file; do not create a new spec or inbox record.{topic}\n{context}\n{repo}\n\nFirst read the existing spec in full. Use the services, files, and paths named in the spec to locate the affected codebases. Inspect the relevant codebases, services, repository instructions, and documentation to validate the spec's assumptions; treat the current code and verified documentation as the source of truth and collect any needed context. Report the validated facts, stale assumptions, and gaps concisely. Then ask what the user wants changed or challenged. Wait for the user's answers before revising the spec, and continue the grill-me interview normally from there. Do not implement code changes, and do not automatically edit the spec before the user answers.\n\nOnce the interview is complete, revise the same spec file at {spec}. Keep the existing title unless the user explicitly asks to rename it. Only after the revised Markdown is complete, run: HERDR_INBOX_HOME={inbox} {executable} finish {id}. If the user explicitly requests a new title, supply --title with the safely shell-quoted new title. Do not mark the spec done before the revised file is complete.",
        profile.skill(),
        record.id,
        record.display_title(),
    ))
}

fn context_references(paths: &[PathBuf]) -> String {
    if paths.is_empty() {
        return String::new();
    }
    let refs = paths
        .iter()
        .map(|path| format!("- {}", shell_quote(&path.to_string_lossy())))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "\nConfigured local context references:\n{refs}\nInspect references relevant to this spec. Keep context at its original location; do not copy it into the Inbox store.\n"
    )
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn shell_paths_round_trip_spaces_quotes_and_substitution() -> Result<()> {
        let path = "/tmp/spec folder/Allar's $(false) `false`.md";
        let output = Command::new("sh")
            .args(["-c", &format!("printf %s {}", shell_quote(path))])
            .output()?;
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout)?, path);
        Ok(())
    }
}
