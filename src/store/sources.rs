//! Where specs live: resolving destinations, relinking items, and relocating a folder.

use super::*;
use std::collections::HashSet;

impl Store {
    pub fn relink(&self, id: &str, path: PathBuf) -> Result<Record> {
        self.locked(|| {
            let mut record = self.get(id)?;
            if record.active_spec_session() {
                return Err("Settle the active spec session before relinking".into());
            }
            let path = absolute(path)?;
            if !path.is_file() {
                return Err("Spec destination must be a readable file".into());
            }
            fs::File::open(&path)?;
            // A scan that saw the moved file first imported it as a fresh item. That
            // placeholder gives way; an item with work on it, or an archived one, does not.
            let mut placeholder = None;
            for other in self.all_records()? {
                if other.id == id || !same_file(&other.spec_path, &path) {
                    continue;
                }
                let active = self.item_path(&other.id)?.is_file();
                if !active || !other.untouched_import() || placeholder.is_some() {
                    return Err(
                        "Another item (including Trash) already references this spec".into(),
                    );
                }
                placeholder = Some(other.id);
            }
            let settings = self.settings()?;
            let location = locate(&settings, &path)?;
            if let Some(placeholder) = placeholder {
                fs::remove_file(self.item_path(&placeholder)?)?;
            }
            record.place(location);
            record.content_fingerprint = fingerprint(&record.spec_path).ok();
            record.updated_at = timestamp();
            self.write(&record)?;
            Ok(record)
        })
    }

    /// Renames a linked spec so its file name starts with the ticket key, for example
    /// `BT-2300-payment-retries.md`. Skills that are given only the key find the spec by it.
    pub(super) fn name_after_ticket(&self, record: &mut Record, settings: &Settings) -> Result<()> {
        let Some(key) = record.jira.key.clone() else {
            return Ok(());
        };
        let path = record.spec_path.clone();
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let slug = slug(if record.title.is_empty() {
            &stem
        } else {
            &record.title
        });
        let mut name = format!("{key}-{slug}");
        if let Some(extension) = path.extension() {
            name = format!("{name}.{}", extension.to_string_lossy());
        }
        let target = path.with_file_name(&name);
        if target == path || !path.is_file() {
            return Ok(());
        }
        if target.symlink_metadata().is_ok() {
            return Err(format!("Cannot name the spec {name}: that file already exists").into());
        }
        // The new name must still belong to the selected folder and match its filters.
        let location = locate(settings, &target)?;
        fs::rename(&path, &target)?;
        record.place(location);
        Ok(())
    }

    /// The caller confirms this is a relocation of the same source, never a replacement source.
    /// The source keeps its ID and filters; items whose file did not move with it keep their
    /// old reference and are reported for explicit relinking.
    pub fn relocate_source(&self, id: &str, path: PathBuf) -> Result<ScanReport> {
        self.locked(|| {
            let mut settings = self.settings()?;
            let index = settings
                .sources
                .iter()
                .position(|source| source.id == id)
                .ok_or("Unknown source ID")?;
            let new_source = SpecSource::new(path)?;
            let all = self.all_records()?;
            let affected: Vec<_> = all.iter().filter(|record| record.source_id == id).collect();
            if affected.iter().any(|record| record.active_spec_session()) {
                return Err(
                    "Settle all affected spec sessions before relocating the source".into(),
                );
            }
            let mut updates = Vec::new();
            let mut issues = Vec::new();
            let mut destinations = HashSet::new();
            for record in affected {
                let relative = &record.source_relative_path;
                let escapes = relative
                    .components()
                    .any(|part| matches!(part, std::path::Component::ParentDir));
                if relative.is_absolute() || escapes {
                    return Err("Invalid stored source-relative path".into());
                }
                let candidate = new_source.path.join(relative);
                let expected = fingerprint(&record.spec_path)
                    .ok()
                    .or_else(|| record.content_fingerprint.clone());
                let actual = fingerprint(&candidate).ok();
                if expected.is_none() || actual.is_none() || expected != actual {
                    issues.push(format!(
                        "{} kept at {}: relocated file missing, unreadable, or content differs; \
                         relink explicitly",
                        record.id,
                        record.spec_path.display()
                    ));
                    continue;
                }
                let canonical = fs::canonicalize(&candidate)?;
                let taken = all
                    .iter()
                    .any(|other| other.id != record.id && same_file(&other.spec_path, &canonical));
                if !destinations.insert(canonical.clone()) || taken {
                    return Err("Relocation would make two items reference the same spec".into());
                }
                let mut updated = record.clone();
                updated.spec_path = canonical;
                updated.content_fingerprint = actual;
                updates.push(updated);
            }
            settings.sources[index].path = new_source.path;
            settings.validate()?;
            for record in updates {
                if self.item_path(&record.id)?.is_file() {
                    self.write(&record)?;
                } else {
                    let archived = self.archived_path(&record.id)?;
                    self.atomic_file(&archived, &serde_json::to_vec_pretty(&record)?)?;
                }
            }
            self.atomic_file(
                &self.settings_path(),
                toml::to_string_pretty(&settings)?.as_bytes(),
            )?;
            let mut report = self.scan_unlocked()?;
            report.issues.extend(issues);
            Ok(report)
        })
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

pub(super) fn fingerprint_bytes(bytes: &[u8]) -> String {
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

/// Resolves a spec path, which may not exist yet, to its place inside a selected folder.
pub(super) fn locate(settings: &Settings, path: &Path) -> Result<Location> {
    let mut existing = path.to_path_buf();
    let mut missing = Vec::new();
    while existing.symlink_metadata().is_err() {
        missing.push(
            existing
                .file_name()
                .ok_or("Invalid spec destination")?
                .to_owned(),
        );
        existing = existing
            .parent()
            .ok_or("Invalid spec destination")?
            .to_path_buf();
    }
    let mut canonical = fs::canonicalize(existing)?;
    for part in missing.into_iter().rev() {
        canonical.push(part);
    }
    if canonical
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("Spec destination cannot contain parent-directory traversal".into());
    }
    if let Some((source, relative)) =
        crate::settings::source_location(&settings.sources, &canonical)
    {
        return Ok(Location {
            source_id: source.id.clone(),
            relative,
            path: canonical,
        });
    }
    Err("Spec destination must be inside a configured spec folder and match its filters".into())
}

/// A file-name-safe form of a title: lowercase words joined by single hyphens.
fn slug(text: &str) -> String {
    let mut slug = String::new();
    for ch in text.chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() {
            slug.push(ch);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug: String = slug.chars().take(60).collect();
    slug.trim_end_matches('-').to_owned()
}
