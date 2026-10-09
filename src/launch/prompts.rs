//! The words that start a session. The agent learns which item and file it owns and the
//! exact command that reports completion back to the Inbox.

use super::{Profile, Record, Result};
use std::env;
use std::path::{Path, PathBuf};

/// Starts the interview for a new spec. The session owns where the spec goes: it knows the
/// folder's layout and the subject, and reports the file it wrote when it finishes.
pub(super) fn initial(
    record: &Record,
    topic: &str,
    profile: Profile,
    data_dir: &Path,
    specs_folder: &Path,
    context_paths: &[PathBuf],
) -> Result<String> {
    let executable = shell_quote(&env::current_exe()?.to_string_lossy());
    let inbox = shell_quote(&data_dir.to_string_lossy());
    let folder = shell_quote(&specs_folder.to_string_lossy());
    let id = shell_quote(&record.id);
    let context = context_references(context_paths);
    let lead = if topic.trim().is_empty() {
        profile.skill().to_string()
    } else {
        format!("{} {}", profile.skill(), topic.trim())
    };
    Ok(format!(
        "{lead}\n{context}\nThis session is inbox item {}. Its title and file do not exist yet; \
         you choose both. Write the finished spec as one Markdown file inside {folder}: use the \
         existing subfolder that fits its subject, add one only when none fits, and give the \
         file a short descriptive name. Identify the affected repositories and services from \
         the conversation and name them in the spec.\n\nWhen the file is complete, choose a \
         concise title and run: HERDR_INBOX_HOME={inbox} {executable} finish {id} --title \
         \"<title>\" --spec <path of the file you wrote>. Do not run it before the file is \
         complete.",
        record.id,
    ))
}

pub(super) fn refinement(
    record: &Record,
    topic: &str,
    profile: Profile,
    data_dir: &Path,
    context_paths: &[PathBuf],
) -> Result<String> {
    let context = context_references(context_paths);
    let executable = shell_quote(&env::current_exe()?.to_string_lossy());
    let spec = shell_quote(&record.spec_path.to_string_lossy());
    let id = shell_quote(&record.id);
    let inbox = shell_quote(&data_dir.to_string_lossy());
    let topic = if topic.trim().is_empty() {
        String::new()
    } else {
        format!("\nInitial context from the user: {}", topic.trim())
    };
    Ok(format!(
        "{}\n\nRefine the existing spec for inbox item {} ({}) at {spec}. Reuse this item and this exact Markdown file; do not create a new spec or inbox record.{topic}\n{context}\n\nFirst read the existing spec in full. Use the services, files, and paths named in the spec to locate the affected codebases. Inspect the relevant codebases, services, repository instructions, and documentation to validate the spec's assumptions; treat the current code and verified documentation as the source of truth and collect any needed context. Report the validated facts, stale assumptions, and gaps concisely. Then ask what the user wants changed or challenged. Wait for the user's answers before revising the spec, and continue the grill-me interview normally from there. Do not implement code changes, and do not automatically edit the spec before the user answers.\n\nOnce the interview is complete, revise the same spec file at {spec}. Keep the existing title unless the user explicitly asks to rename it. Only after the revised Markdown is complete, run: HERDR_INBOX_HOME={inbox} {executable} finish {id}. If the user explicitly requests a new title, supply --title with the safely shell-quoted new title. Do not mark the spec done before the revised file is complete.",
        profile.skill(),
        record.id,
        record.display_title(),
    ))
}

/// Asks for a Jira ticket created from the spec, and for the result to be reported back.
pub(super) fn ticket(record: &Record, parent: Option<&str>, data_dir: &Path) -> Result<String> {
    let executable = shell_quote(&env::current_exe()?.to_string_lossy());
    let inbox = shell_quote(&data_dir.to_string_lossy());
    let spec = shell_quote(&record.spec_path.to_string_lossy());
    let id = shell_quote(&record.id);
    let placement = match parent {
        Some(parent) => format!(
            "Create it under {parent}, in the same project: as a sub-task when {parent} is a \
             story or task, otherwise as a child issue of that epic."
        ),
        None => "It has no parent issue; ask which project to create it in if that is unclear."
            .to_owned(),
    };
    Ok(format!(
        "Create a Jira issue for inbox item {} ({}) from the finished spec at {spec}. Use your \
         Jira tools. {placement} Write a short summary line, and a description with the spec's \
         goal, key requirements and acceptance criteria; do not paste the whole spec. Do not \
         edit the spec.\n\nWhen the issue exists, run: HERDR_INBOX_HOME={inbox} {executable} jira \
         {id} <ISSUE-KEY> --url <issue URL>. That command links the issue and renames the spec \
         file to start with the issue key; it prints the new path, which replaces {spec} from \
         then on. If you cannot create the issue, explain why and do not run the command.",
        record.id,
        record.display_title(),
    ))
}

/// Invokes the development skill on the item and asks for its branch and draft PR.
pub(super) fn develop(record: &Record, skill: &str, data_dir: &Path) -> Result<String> {
    let executable = shell_quote(&env::current_exe()?.to_string_lossy());
    let inbox = shell_quote(&data_dir.to_string_lossy());
    let spec = shell_quote(&record.spec_path.to_string_lossy());
    let id = shell_quote(&record.id);
    // The skill is handed the ticket key, or the spec itself when there is no ticket.
    let subject = record.jira.key.clone().unwrap_or_else(|| spec.clone());
    Ok(format!(
        "{skill} {subject}\n\nThis is inbox item {} ({}); its spec is {spec}. Report progress to \
         the Inbox as you go. Once you are working on a branch, run: HERDR_INBOX_HOME={inbox} \
         {executable} implement {id} --agent {} --branch <branch>. Once a draft PR exists, run: \
         HERDR_INBOX_HOME={inbox} {executable} pr {id} <PR URL>.",
        record.id,
        record.display_title(),
        shell_quote(skill.trim_start_matches(['/', '$'])),
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
