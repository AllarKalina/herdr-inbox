use crate::store::Result;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Folder,
    Context,
}

// Paths arrive through argv and leave as JSON; neither shell nor JavaScript
// interprets a user-selected filename.
const SCRIPT: &str = r#"
ObjC.import('AppKit');
function run(argv) {
    const context = argv[0] === 'context';
    const app = $.NSApplication.sharedApplication;
    app.setActivationPolicy(Number($.NSApplicationActivationPolicyAccessory));
    const panel = $.NSOpenPanel.openPanel;
    panel.title = context ? 'Choose context file or folder' : 'Choose specs folder';
    panel.prompt = 'Choose';
    panel.canChooseDirectories = true;
    panel.canChooseFiles = context;
    panel.allowsMultipleSelection = false;
    panel.resolvesAliases = true;
    panel.canCreateDirectories = false;
    if (argv[1]) {
        panel.directoryURL = $.NSURL.fileURLWithPath(argv[1]);
    }
    app.activateIgnoringOtherApps(true);
    panel.makeKeyAndOrderFront(null);
    if (Number(panel.runModal) !== Number($.NSModalResponseOK)) {
        return JSON.stringify(null);
    }
    return JSON.stringify(ObjC.unwrap(panel.URL.path));
}
"#;

fn command(kind: Kind, initial: Option<&Path>) -> Result<Command> {
    let initial = initial.map(|path| {
        if path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        }
    });
    let initial = initial
        .map(|path| {
            path.to_str()
                .ok_or("The initial folder path is not valid Unicode; choose another folder")
        })
        .transpose()?;
    let mut command = Command::new("/usr/bin/osascript");
    command.args(["-l", "JavaScript", "-e", SCRIPT]);
    command.arg(match kind {
        Kind::Folder => "folder",
        Kind::Context => "context",
    });
    command.arg(initial.unwrap_or(""));
    Ok(command)
}

fn decode(output: Output) -> Result<Option<PathBuf>> {
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim();
        return Err(format!(
            "Could not open the macOS selector: {}. Try again or enter the path manually",
            if detail.is_empty() {
                "osascript did not finish successfully"
            } else {
                detail
            }
        )
        .into());
    }
    let path: Option<String> = serde_json::from_slice(&output.stdout).map_err(
        |_| "The macOS selector returned an invalid path; try again or enter the path manually",
    )?;
    path.map(|value| {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            Ok(path)
        } else {
            Err("The macOS selector returned a non-absolute path; choose another location".into())
        }
    })
    .transpose()
}

#[cfg(not(test))]
pub(super) fn choose(kind: Kind, initial: Option<&Path>) -> Result<Option<PathBuf>> {
    let output = command(kind, initial)?.output().map_err(|error| {
        format!("Could not start the macOS selector: {error}. Enter the path manually instead")
    })?;
    decode(output)
}

#[cfg(test)]
std::thread_local! {
    static TEST_RESULTS: std::cell::RefCell<std::collections::VecDeque<Result<Option<PathBuf>>>> =
        const { std::cell::RefCell::new(std::collections::VecDeque::new()) };
}

#[cfg(test)]
pub(crate) fn set_test_result(result: Result<Option<PathBuf>>) {
    TEST_RESULTS.with(|results| results.borrow_mut().push_back(result));
}

#[cfg(test)]
pub(super) fn choose(_kind: Kind, _initial: Option<&Path>) -> Result<Option<PathBuf>> {
    TEST_RESULTS.with(|results| results.borrow_mut().pop_front().unwrap_or(Ok(None)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    fn output(stdout: Vec<u8>, success: bool, stderr: &str) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(if success { 0 } else { 1 << 8 }),
            stdout,
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[test]
    fn selected_path_keeps_whitespace_unicode_and_special_characters() {
        let path = "/tmp/ User's \"specs\" /日本語\n$(touch never)";
        let json = serde_json::to_vec(path).unwrap();
        assert_eq!(decode(output(json, true, "")).unwrap(), Some(path.into()));
    }

    #[test]
    fn cancellation_has_no_path() {
        assert_eq!(decode(output(b"null\n".to_vec(), true, "")).unwrap(), None);
    }

    #[test]
    fn failures_are_actionable_without_being_treated_as_cancellation() {
        let error = decode(output(Vec::new(), false, "GUI unavailable"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("GUI unavailable"));
        assert!(error.contains("enter the path manually"));
        for invalid in ["", "broken JSON", "42", "\"relative/path\"", "\"\""] {
            assert!(decode(output(invalid.as_bytes().to_vec(), true, "")).is_err());
        }
    }

    #[test]
    fn path_is_passed_as_an_argument_to_a_static_script() {
        let path = Path::new("/tmp/'\"日本語\n$(touch never)");
        let command = command(Kind::Context, Some(path)).unwrap();
        assert_eq!(command.get_program(), "/usr/bin/osascript");
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args[3], SCRIPT);
        assert_eq!(args[4], "context");
        assert_eq!(args[5], path.as_os_str());
        assert!(!SCRIPT.contains(path.to_str().unwrap()));
        assert!(SCRIPT.contains("panel.canChooseFiles = context"));
        assert!(SCRIPT.contains("panel.canChooseDirectories = true"));
        assert!(SCRIPT.contains("panel.allowsMultipleSelection = false"));
        assert!(SCRIPT.contains("JSON.stringify(ObjC.unwrap(panel.URL.path))"));
    }

    #[test]
    fn folder_command_and_mock_results_do_not_open_a_native_panel() {
        let command = command(Kind::Folder, None).unwrap();
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args[4], "folder");
        assert_eq!(args[5], "");
        set_test_result(Ok(Some("/tmp/specs".into())));
        assert_eq!(
            choose(Kind::Folder, None).unwrap(),
            Some("/tmp/specs".into())
        );
        assert_eq!(choose(Kind::Folder, None).unwrap(), None);
        set_test_result(Err("picker failed".into()));
        assert!(choose(Kind::Context, None).is_err());
    }

    #[test]
    fn existing_context_file_starts_the_panel_in_its_parent_folder() {
        let root = std::env::temp_dir().join(format!("herdr-picker-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let file = root.join("context notes.md");
        std::fs::write(&file, "context").unwrap();
        let command = command(Kind::Context, Some(&file)).unwrap();
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args[5], root.as_os_str());
        // Only the starting directory changes; a selected file remains a file.
        let selected = serde_json::to_vec(file.to_str().unwrap()).unwrap();
        assert_eq!(decode(output(selected, true, "")).unwrap(), Some(file));
        std::fs::remove_dir_all(root).unwrap();
    }
}
