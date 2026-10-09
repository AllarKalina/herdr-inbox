//! Archived specs: hidden from the Inbox until restored, or deleted for good.

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
    pub fn archive(&self, id: &str) -> Result<Record> {
        self.locked(|| {
            let record = self.get(id)?;
            let item_trash = self.archived_path(id)?;
            if item_trash.symlink_metadata().is_ok() {
                return Err("Archive already contains this item; restore it first".into());
            }
            ensure_dir(item_trash.parent().ok_or("Invalid archive item path")?)?;
            fs::rename(self.item_path(id)?, &item_trash)?;
            Ok(record)
        })
    }

    pub fn archived(&self) -> Result<Vec<Record>> {
        let mut records = Vec::new();
        let trash = self.archive_dir();
        if trash.is_dir() {
            for entry in fs::read_dir(trash)? {
                let path = entry?.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    records.extend(self.read_listed(&path)?);
                }
            }
        }
        Ok(records)
    }

    pub(super) fn all_records(&self) -> Result<Vec<Record>> {
        let mut records = self.list()?;
        records.extend(self.archived()?);
        Ok(records)
    }

    pub fn restore(&self, id: &str) -> Result<Record> {
        self.locked(|| {
            let path = self.archived_path(id)?;
            let record = self.read_record(&path)?;
            if self.item_path(id)?.symlink_metadata().is_ok() {
                return Err("Item already exists".into());
            }
            if self
                .list()?
                .iter()
                .any(|r| same_file(&r.spec_path, &record.spec_path))
            {
                return Err("Another item already references this spec".into());
            }
            ensure_dir(&self.items_dir())?;
            fs::rename(&path, self.item_path(id)?)?;
            Ok(record)
        })
    }

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
