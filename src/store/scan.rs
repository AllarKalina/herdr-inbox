//! Discovery: reconciling records with the files inside the selected folders.

use super::*;
use std::collections::HashSet;

#[derive(Debug, Default, Serialize)]
pub struct ScanReport {
    pub imported: usize,
    pub known: usize,
    pub archived: usize,
    /// Records removed because their spec file is gone, or archived from another folder.
    pub dropped: usize,
    pub issues: Vec<String>,
}

impl ScanReport {
    pub fn summary(&self) -> String {
        let dropped = if self.dropped > 0 {
            format!(", {} dropped", self.dropped)
        } else {
            String::new()
        };
        format!(
            "{} imported, {} known, {} archived{dropped}; {} issue(s)",
            self.imported,
            self.known,
            self.archived,
            self.issues.len()
        )
    }
}

impl Store {
    pub fn scan(&self) -> Result<super::ScanReport> {
        self.locked(|| self.scan_unlocked())
    }

    /// Removes records the selected folders no longer back: an Inbox item whose spec file was
    /// deleted from its folder, and an archived spec whose file is gone or lies outside the
    /// selected folders.
    /// Only metadata is removed. A folder that cannot be read proves nothing about its files,
    /// so its records are left alone, as is a new spec whose session has not written it yet.
    fn prune(&self, settings: &Settings) -> Result<usize> {
        let source_of = |record: &Record| {
            settings
                .sources
                .iter()
                .find(|source| record.source_id == source.id)
        };
        let readable =
            |record: &Record| source_of(record).map(|source| fs::read_dir(&source.path).is_ok());
        // A record left behind by a relocated folder still points at the old location;
        // it waits for an explicit relink instead of being treated as deleted.
        let expected_in_folder = |record: &Record| {
            source_of(record)
                .and_then(|source| fs::canonicalize(&source.path).ok())
                .is_some_and(|root| record.spec_path.starts_with(root))
        };
        let gone = |record: &Record| matches!(record.spec_path.symlink_metadata(), Err(error) if error.kind() == io::ErrorKind::NotFound);
        let mut dropped = 0;
        for record in self.list()? {
            if readable(&record) == Some(true)
                && expected_in_folder(&record)
                && gone(&record)
                && !record.active_spec_session()
            {
                fs::remove_file(self.item_path(&record.id)?)?;
                dropped += 1;
            }
        }
        for record in self.archived()? {
            if readable(&record) == Some(false) {
                continue;
            }
            let inside = fs::canonicalize(&record.spec_path)
                .ok()
                .filter(|path| path.is_file())
                .is_some_and(|path| {
                    crate::settings::source_location(&settings.sources, &path).is_some()
                });
            if !inside {
                fs::remove_file(self.archived_path(&record.id)?)?;
                dropped += 1;
            }
        }
        Ok(dropped)
    }

    pub(super) fn scan_unlocked(&self) -> Result<ScanReport> {
        let settings = self.settings()?;
        let dropped = self.prune(&settings)?;
        let mut known = self.list()?;
        let all = self.all_records()?;
        let trash: Vec<_> = all
            .iter()
            .filter(|r| !known.iter().any(|k| k.id == r.id))
            .collect();
        let mut report = ScanReport {
            dropped,
            ..ScanReport::default()
        };
        let mut seen_files = HashSet::new();
        for source in &settings.sources {
            let (include, exclude) = source.filters()?;
            let mut seen_dirs = HashSet::new();
            let mut candidates = Vec::new();
            discover(
                source,
                &source.path,
                &include,
                &exclude,
                &mut seen_dirs,
                &mut candidates,
                &mut report.issues,
            );
            candidates.sort();
            for (path, _) in candidates {
                let resolved = match fs::canonicalize(&path) {
                    Ok(path) => path,
                    Err(error) => {
                        report
                            .issues
                            .push(format!("Cannot resolve {}: {error}", path.display()));
                        continue;
                    }
                };
                let source_root = fs::canonicalize(&source.path)?;
                if !resolved.starts_with(source_root) {
                    report
                        .issues
                        .push(format!("Spec outside selected folder: {}", path.display()));
                    continue;
                }
                let Some((source, relative)) =
                    crate::settings::source_location(&settings.sources, &resolved)
                else {
                    continue;
                };
                if !seen_files.insert(resolved.clone()) {
                    continue;
                }
                if known
                    .iter()
                    .any(|record| same_file(&record.spec_path, &resolved))
                {
                    report.known += 1;
                    continue;
                }
                if trash
                    .iter()
                    .any(|record| same_file(&record.spec_path, &resolved))
                {
                    report.archived += 1;
                    continue;
                }
                let text = match fs::read_to_string(&resolved) {
                    Ok(text) => text,
                    Err(error) => {
                        report
                            .issues
                            .push(format!("Cannot read spec {}: {error}", path.display()));
                        continue;
                    }
                };
                let title = text
                    .lines()
                    .find_map(|line| line.trim_start().strip_prefix("# ").map(str::trim))
                    .filter(|title| !title.is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        path.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned()
                    });
                let location = Location {
                    source_id: source.id.clone(),
                    relative,
                    path: resolved,
                };
                let record = Record {
                    content_fingerprint: Some(fingerprint_bytes(text.as_bytes())),
                    title,
                    ..Record::new(
                        Uuid::new_v4().to_string(),
                        location,
                        SpecStatus::Done,
                        timestamp(),
                    )
                };
                self.write(&record)?;
                known.push(record);
                report.imported += 1;
            }
        }
        for record in &known {
            // A new spec session has no file until its agent writes one, and records from
            // folders that are no longer selected are not this scan's concern.
            let selected = settings
                .sources
                .iter()
                .any(|source| record.source_id == source.id);
            if record.active_spec_session() || !selected {
                continue;
            }
            if let Err(error) = fs::File::open(&record.spec_path) {
                report.issues.push(format!(
                    "Spec unavailable for {}: {} ({error})",
                    record.id,
                    record.spec_path.display()
                ));
            }
        }
        Ok(report)
    }
}

fn discover(
    source: &SpecSource,
    directory: &Path,
    include: &globset::GlobSet,
    exclude: &globset::GlobSet,
    seen: &mut HashSet<PathBuf>,
    files: &mut Vec<(PathBuf, PathBuf)>,
    issues: &mut Vec<String>,
) {
    let resolved = match fs::canonicalize(directory) {
        Ok(path) => path,
        Err(error) => {
            issues.push(format!(
                "Source unavailable: {} ({error})",
                directory.display()
            ));
            return;
        }
    };
    if fs::canonicalize(&source.path).is_ok_and(|root| !resolved.starts_with(root)) {
        issues.push(format!(
            "Directory outside selected folder: {}",
            directory.display()
        ));
        return;
    }
    if !seen.insert(resolved) {
        return;
    }
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            issues.push(format!("Cannot scan {}: {error}", directory.display()));
            return;
        }
    };
    let mut entries: Vec<_> = entries.collect();
    entries.sort_by_key(|entry| entry.as_ref().map(|e| e.path()).unwrap_or_default());
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                issues.push(error.to_string());
                continue;
            }
        };
        let path = entry.path();
        let Ok(relative) = path.strip_prefix(&source.path) else {
            continue;
        };
        if exclude.is_match(relative) {
            continue;
        }
        match fs::metadata(&path) {
            Ok(metadata) if metadata.is_dir() && source.recursive => {
                discover(source, &path, include, exclude, seen, files, issues)
            }
            Ok(metadata) if metadata.is_file() && include.is_match(relative) => {
                files.push((path.clone(), relative.to_path_buf()))
            }
            Ok(_) => {}
            Err(error) => issues.push(format!("Cannot inspect {}: {error}", path.display())),
        }
    }
}
