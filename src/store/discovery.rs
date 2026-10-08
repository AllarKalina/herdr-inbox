use super::*;
use std::collections::HashSet;

#[derive(Debug, Default, Serialize)]
pub struct ScanReport {
    pub imported: usize,
    pub known: usize,
    pub suppressed: usize,
    pub issues: Vec<String>,
}

impl ScanReport {
    pub fn summary(&self) -> String {
        format!(
            "{} imported, {} known, {} in Trash; {} issue(s)",
            self.imported,
            self.known,
            self.suppressed,
            self.issues.len()
        )
    }
}

impl Store {
    pub(super) fn all_records(&self) -> Result<Vec<Record>> {
        let mut records = self.list()?;
        records.extend(self.trash_list()?);
        Ok(records)
    }

    pub fn trash_list(&self) -> Result<Vec<Record>> {
        let mut records = Vec::new();
        let trash = self.root.join("trash/items");
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

    pub fn scan(&self) -> Result<super::ScanReport> {
        self.locked(|| self.scan_unlocked())
    }

    fn scan_unlocked(&self) -> Result<ScanReport> {
        let settings = self.settings()?;
        let mut known = self.list()?;
        let all = self.all_records()?;
        let trash: Vec<_> = all
            .iter()
            .filter(|r| !known.iter().any(|k| k.id == r.id))
            .collect();
        let mut report = ScanReport::default();
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
                    report.suppressed += 1;
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
                let record = Record {
                    source_id: Some(source.id.clone()),
                    source_relative_path: Some(relative),
                    content_fingerprint: Some(fingerprint_bytes(text.as_bytes())),
                    title,
                    ..Record::blank(
                        Uuid::new_v4().to_string(),
                        resolved,
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
                .any(|source| record.source_id.as_deref() == Some(source.id.as_str()));
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

    /// The caller confirms this is a relocation of the same source, never a replacement source.
    pub fn relocate_source(&self, id: &str, path: PathBuf) -> Result<ScanReport> {
        self.locked(|| {
            let mut settings = self.settings()?;
            let index = settings.sources.iter().position(|s| s.id == id).ok_or("Unknown source ID")?;
            let new_source = SpecSource::new(path)?;
            let old_source = &settings.sources[index];
            let all = self.all_records()?;
            let affected: Vec<_> = all.iter().filter(|r| r.source_id.as_deref() == Some(id)).collect();
            if affected.iter().any(|r| r.active_spec_session()) {
                return Err("Settle all affected spec sessions before relocating the source".into());
            }
            let mut updates = Vec::new();
            let mut issues = Vec::new();
            let mut destinations = HashSet::new();
            for record in affected {
                let Some(relative) = &record.source_relative_path else { continue; };
                if relative.is_absolute() || relative.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                    return Err("Invalid stored source-relative path".into());
                }
                let candidate = new_source.path.join(relative);
                let expected = fingerprint(&record.spec_path).ok().or_else(|| record.content_fingerprint.clone());
                let actual = fingerprint(&candidate).ok();
                if expected.is_none() || actual.is_none() || expected != actual {
                    issues.push(format!("{} kept at {}: relocated file missing, unreadable, or content differs; relink explicitly", record.id, record.spec_path.display()));
                    continue;
                }
                let canonical = fs::canonicalize(&candidate)?;
                if !destinations.insert(canonical.clone()) || all.iter().any(|other| other.id != record.id && same_file(&other.spec_path, &canonical)) {
                    return Err("Relocation would make two items reference the same spec".into());
                }
                let mut updated = record.clone();
                updated.spec_path = canonical;
                updated.content_fingerprint = actual;
                updates.push(updated);
            }
            // Existing settings retain source ID and filters. Unmatched items retain their old references.
            let _ = old_source;
            settings.sources[index].path = new_source.path;
            settings.validate()?;
            for record in updates {
                if self.path_for(&record.id)?.is_file() { self.write(&record)?; }
                else {
                    let path = self.root.join("trash/items").join(format!("{}.json", record.id));
                    self.atomic_file(&path, &serde_json::to_vec_pretty(&record)?)?;
                }
            }
            self.atomic_file(&self.settings_path(), toml::to_string_pretty(&settings)?.as_bytes())?;
            let mut report = self.scan_unlocked()?;
            report.issues.extend(issues);
            Ok(report)
        })
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

pub(super) fn same_file(a: &Path, b: &Path) -> bool {
    a == b
        || match (fs::canonicalize(a), fs::canonicalize(b)) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
}

pub(super) fn fingerprint(path: &Path) -> Result<String> {
    Ok(fingerprint_bytes(&fs::read(path)?))
}
fn fingerprint_bytes(bytes: &[u8]) -> String {
    let mut a = 0xcbf29ce484222325_u64;
    let mut b = 0x84222325cbf29ce4_u64;
    for byte in bytes {
        a = (a ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
        b = (b ^ u64::from(*byte))
            .wrapping_mul(0x100000001b3)
            .rotate_left(7);
    }
    format!("{:016x}{:016x}:{}", a, b, bytes.len())
}
