use super::*;

// The path arrives through argv; the script text never contains a filename.
#[cfg(not(test))]
const SCRIPT: &str = r#"
ObjC.import('Foundation');
function run(argv) {
    const url = $.NSURL.fileURLWithPath(argv[0]);
    const moved = $.NSFileManager.defaultManager
        .trashItemAtURLResultingItemURLError(url, null, null);
    if (!moved) {
        throw new Error('macOS refused to move the file');
    }
}
"#;

#[cfg(not(test))]
fn move_to_macos_trash(_root: &Path, path: &Path) -> Result<()> {
    let output = std::process::Command::new("/usr/bin/osascript")
        .args(["-l", "JavaScript", "-e", SCRIPT])
        .arg(path)
        .output()
        .map_err(|error| format!("Could not start the macOS Trash helper: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Could not move {} to the macOS Trash: {}",
            path.display(),
            detail.trim()
        )
        .into());
    }
    Ok(())
}

/// Tests keep deleted specs under the temporary data root, never in the real Trash.
#[cfg(test)]
fn move_to_macos_trash(root: &Path, path: &Path) -> Result<()> {
    let trash = root.join("test-macos-trash");
    ensure_dir(&trash)?;
    fs::rename(
        path,
        trash.join(path.file_name().ok_or("Invalid spec path")?),
    )?;
    Ok(())
}

impl Store {
    /// Deletes an archived record and moves its spec file to the macOS Trash.
    /// Returns whether a file was moved: it stays when it is already gone or when an
    /// Inbox item still uses it.
    pub fn delete_archived(&self, id: &str) -> Result<(Record, bool)> {
        self.locked(|| {
            let path = self.archived_path(id)?;
            let record = self.read_record(&path)?;
            let in_use = self
                .list()?
                .iter()
                .any(|other| same_file(&other.spec_path, &record.spec_path));
            // The file goes first: a failed move must leave the archived record recoverable.
            let moved = !in_use && record.spec_path.symlink_metadata().is_ok();
            if moved {
                move_to_macos_trash(&self.root, &record.spec_path)?;
            }
            fs::remove_file(&path)?;
            Ok((record, moved))
        })
    }
}
