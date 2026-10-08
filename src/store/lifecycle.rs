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
        let mut record: Record = serde_json::from_slice(&fs::read(path)?)?;
        if record.schema_version > 1 {
            return Err(format!(
                "Unsupported metadata schema {} in {}",
                record.schema_version,
                path.display()
            )
            .into());
        }
        if record.schema_version == 0 {
            record.schema_version = 1;
            if record.spec == "done"
                && let Some(launch) = &mut record.launch
                && launch.status == "prompt_sent"
            {
                launch.status = "completed".into();
            }
            record.ownership =
                if record.spec_path == self.root.join("specs").join(format!("{}.md", record.id)) {
                    "managed"
                } else {
                    "user"
                }
                .into();
        }
        if !matches!(record.ownership.as_str(), "managed" | "user") {
            return Err(format!("Invalid file ownership in {}", path.display()).into());
        }
        Ok(record)
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
            let explicit = spec.is_some();
            let source = settings.sources.first();
            let path = match spec {
                Some(path) => absolute(path)?,
                None => source
                    .map(|s| s.path.join(format!("{id}.md")))
                    .unwrap_or_else(|| self.root.join("specs").join(format!("{id}.md"))),
            };
            if !create_spec && path.symlink_metadata().is_ok() {
                return Err(
                    "New spec destination already exists; import it or refine its existing item"
                        .into(),
                );
            }
            if explicit
                && self
                    .all_records()?
                    .iter()
                    .any(|record| same_file(&record.spec_path, &path))
            {
                return Err(
                    "This spec already has metadata; use its existing item or restore it".into(),
                );
            }
            if !explicit && source.is_some_and(|s| !s.path.is_dir()) {
                return Err("Configured spec source is unavailable; correct it in settings".into());
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
            let ownership = if explicit || source.is_some() {
                "user"
            } else {
                "managed"
            };
            let association = settings.sources.iter().find_map(|s| {
                path.strip_prefix(&s.path)
                    .ok()
                    .map(|relative| (s.id.clone(), relative.to_path_buf()))
            });
            let now = timestamp();
            let record = Record {
                schema_version: 1,
                ownership: ownership.into(),
                source_id: association.as_ref().map(|a| a.0.clone()),
                source_relative_path: association.map(|a| a.1),
                content_fingerprint: fingerprint(&path).ok(),
                id,
                title: title.to_owned(),
                repo: repo.map(absolute).transpose()?,
                spec_path: path,
                created_at: now,
                updated_at: now,
                spec: "in_progress".into(),
                jira: Link {
                    status: "waiting".into(),
                    key: None,
                    url: None,
                },
                implementation: Implementation {
                    status: "waiting".into(),
                    agent: None,
                    branch: None,
                },
                pr: Link {
                    status: "waiting".into(),
                    key: None,
                    url: None,
                },
                launch: None,
                previous_launches: Vec::new(),
            };
            self.write(&record)?;
            Ok(record)
        })
    }

    /// Acknowledges that the caller has ended or abandoned the local spec session.
    pub fn settle(&self, id: &str) -> Result<Record> {
        self.locked(|| {
            let mut record = self.get(id)?;
            if let Some(launch) = &mut record.launch {
                launch.status = "completed".into();
            }
            record.updated_at = timestamp();
            self.write(&record)?;
            Ok(record)
        })
    }

    pub fn restore(&self, id: &str) -> Result<Record> {
        self.locked(|| {
            Uuid::parse_str(id)?;
            let path = self.root.join("trash/items").join(format!("{id}.json"));
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
            let spec_trash = self.root.join("trash/specs").join(format!("{id}.md"));
            let restore_spec = self.manages_spec(&record) && spec_trash.is_file();
            if restore_spec {
                if record.spec_path.symlink_metadata().is_ok() {
                    return Err(
                        "Original spec location already exists; refusing to overwrite".into(),
                    );
                }
                ensure_dir(record.spec_path.parent().ok_or("Invalid spec path")?)?;
                fs::rename(&spec_trash, &record.spec_path)?;
            }
            ensure_dir(&self.items())?;
            if let Err(error) = fs::rename(&path, self.path_for(id)?) {
                if restore_spec {
                    let _ = fs::rename(&record.spec_path, spec_trash);
                }
                return Err(error.into());
            }
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
            if self
                .all_records()?
                .iter()
                .any(|other| other.id != id && same_file(&other.spec_path, &path))
            {
                return Err("Another item (including Trash) already references this spec".into());
            }
            record.spec_path = path;
            record.ownership = "user".into();
            let settings = self.settings()?;
            let association = settings.sources.iter().find_map(|s| {
                record
                    .spec_path
                    .strip_prefix(&s.path)
                    .ok()
                    .map(|relative| (s.id.clone(), relative.to_path_buf()))
            });
            record.source_id = association.as_ref().map(|a| a.0.clone());
            record.source_relative_path = association.map(|a| a.1);
            record.content_fingerprint = fingerprint(&record.spec_path).ok();
            record.updated_at = timestamp();
            self.write(&record)?;
            Ok(record)
        })
    }
}
