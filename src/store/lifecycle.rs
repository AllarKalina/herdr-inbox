use super::*;

impl Store {
    pub fn settings_path(&self) -> PathBuf {
        self.root.join("settings.toml")
    }

    pub fn settings(&self) -> Result<Settings> {
        match fs::read_to_string(self.settings_path()) {
            Ok(text) => {
                let mut settings: Settings = toml::from_str(&text)?;
                settings.validate()?;
                Ok(settings)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Settings::default()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        self.locked(|| {
            // A newer installed version's configuration must never be replaced blindly.
            self.settings()?;
            self.write_settings(settings)
        })
    }

    /// Applies a change to the current settings under the store lock.
    pub fn update_settings<T>(&self, change: impl FnOnce(&mut Settings) -> Result<T>) -> Result<T> {
        self.locked(|| {
            let mut settings = self.settings()?;
            let value = change(&mut settings)?;
            self.write_settings(&settings)?;
            Ok(value)
        })
    }

    pub(super) fn write_settings(&self, settings: &Settings) -> Result<()> {
        let mut validated = settings.clone();
        validated.validate()?;
        let old = self.settings()?;
        for source in &validated.sources {
            if let Some(previous) = old.sources.iter().find(|s| s.id == source.id)
                && previous.path != source.path
            {
                return Err(
                    "Use explicit source relocation to change an existing source's path".into(),
                );
            }
        }
        self.atomic_file(
            &self.settings_path(),
            toml::to_string_pretty(&validated)?.as_bytes(),
        )
    }

    pub(super) fn atomic_file(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        let parent = path.parent().ok_or("Invalid destination")?;
        ensure_dir(parent)?;
        let temporary = parent.join(format!(".{}.tmp", Uuid::new_v4()));
        let result = (|| -> Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            fs::rename(&temporary, path)?;
            fs::File::open(parent)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result
    }

    pub(super) fn read_record(&self, path: &Path) -> Result<Record> {
        let record: Record = serde_json::from_slice(&fs::read(path)?)?;
        if record.schema_version != 1 {
            return Err(format!(
                "Unsupported metadata schema {} in {}",
                record.schema_version,
                path.display()
            )
            .into());
        }
        Ok(record)
    }

    /// Reads a record found by a directory listing. Listings run without the lock, so a
    /// record archived or restored in between has simply moved and is not an error.
    pub(super) fn read_listed(&self, path: &Path) -> Result<Option<Record>> {
        match self.read_record(path) {
            Ok(record) => Ok(Some(record)),
            Err(error)
                if error
                    .downcast_ref::<io::Error>()
                    .is_some_and(|error| error.kind() == io::ErrorKind::NotFound) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    pub fn start_untitled_with_spec(
        &self,
        repo: Option<PathBuf>,
        spec: Option<PathBuf>,
    ) -> Result<Record> {
        self.create("", repo, spec, false)
    }

    pub(super) fn create(
        &self,
        title: &str,
        repo: Option<PathBuf>,
        spec: Option<PathBuf>,
        create_spec: bool,
    ) -> Result<Record> {
        self.locked(|| {
            let id = Uuid::new_v4().to_string();
            let settings = self.settings()?;
            let path = match spec {
                Some(path) => absolute(path)?,
                None => {
                    let source = settings
                        .sources
                        .first()
                        .ok_or("Choose a spec folder in settings before creating a spec")?;
                    if !source.path.is_dir() {
                        return Err(
                            "Configured spec source is unavailable; correct it in settings".into(),
                        );
                    }
                    source.path.join(format!("{id}.md"))
                }
            };
            let (source_id, relative, path) = self.destination_source(&settings, &path)?;
            if !create_spec && path.symlink_metadata().is_ok() {
                return Err(
                    "New spec destination already exists; import it or refine its existing item"
                        .into(),
                );
            }
            if self
                .all_records()?
                .iter()
                .any(|record| same_file(&record.spec_path, &path))
            {
                return Err(
                    "This spec already has metadata; use its existing item or restore it".into(),
                );
            }
            if let Some(parent) = path.parent() {
                ensure_dir(parent)?;
            }
            if create_spec && !path.exists() {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&path)?;
                writeln!(file, "# {title}\n")?;
                file.sync_all()?;
            }
            let record = Record {
                source_id: Some(source_id),
                source_relative_path: Some(relative),
                content_fingerprint: fingerprint(&path).ok(),
                title: title.to_owned(),
                repo: repo.map(absolute).transpose()?,
                ..Record::blank(id, path, SpecStatus::InProgress, timestamp())
            };
            self.write(&record)?;
            Ok(record)
        })
    }

    fn destination_source(
        &self,
        settings: &Settings,
        path: &Path,
    ) -> Result<(String, PathBuf, PathBuf)> {
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
            return Ok((source.id.clone(), relative, canonical));
        }
        Err("Spec destination must be inside a configured spec folder and match its filters".into())
    }

    /// Removes the record of a new spec whose session never opened a tab. Nothing else
    /// refers to it yet, and its spec file was never written.
    pub fn discard_unstarted(&self, id: &str) -> Result<()> {
        self.locked(|| {
            let record = self.get(id)?;
            if record.spec_path.symlink_metadata().is_ok() {
                return Err("Spec file exists; archive the item instead".into());
            }
            fs::remove_file(self.path_for(id)?)?;
            Ok(())
        })
    }

    /// Acknowledges that the caller has ended or abandoned the local spec session.
    pub fn settle(&self, id: &str) -> Result<Record> {
        self.locked(|| {
            let mut record = self.get(id)?;
            if let Some(launch) = &mut record.launch {
                launch.status = LaunchStatus::Completed;
            }
            record.updated_at = timestamp();
            self.write(&record)?;
            Ok(record)
        })
    }

    pub fn restore(&self, id: &str) -> Result<Record> {
        self.locked(|| {
            let path = self.archived_path(id)?;
            let record = self.read_record(&path)?;
            if self.path_for(id)?.symlink_metadata().is_ok() {
                return Err("Item already exists".into());
            }
            if self
                .list()?
                .iter()
                .any(|r| same_file(&r.spec_path, &record.spec_path))
            {
                return Err("Another item already references this spec".into());
            }
            ensure_dir(&self.items())?;
            fs::rename(&path, self.path_for(id)?)?;
            Ok(record)
        })
    }

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
                let active = self.path_for(&other.id)?.is_file();
                if !active || !other.untouched_import() || placeholder.is_some() {
                    return Err(
                        "Another item (including Trash) already references this spec".into(),
                    );
                }
                placeholder = Some(other.id);
            }
            let settings = self.settings()?;
            let (source_id, relative, path) = self.destination_source(&settings, &path)?;
            if let Some(placeholder) = placeholder {
                fs::remove_file(self.path_for(&placeholder)?)?;
            }
            record.spec_path = path;
            record.source_id = Some(source_id);
            record.source_relative_path = Some(relative);
            record.content_fingerprint = fingerprint(&record.spec_path).ok();
            record.updated_at = timestamp();
            self.write(&record)?;
            Ok(record)
        })
    }
}
