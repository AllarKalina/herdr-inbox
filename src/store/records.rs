//! Item records: creating, reading, and changing them.

use super::*;

impl Store {
    /// Records a spec with a title and an empty file to write it in.
    pub fn start(&self, title: &str, spec: Option<PathBuf>) -> Result<Record> {
        let title = title.trim();
        if title.is_empty() {
            return Err("Title cannot be empty".into());
        }
        self.create(title, spec, true)
    }

    /// Records a new spec whose session will supply its title, its file, and where that
    /// file lives. Until then the item holds a provisional path that nothing is written to.
    pub fn start_untitled(&self) -> Result<Record> {
        self.create("", None, false)
    }

    fn create(&self, title: &str, spec: Option<PathBuf>, create_spec: bool) -> Result<Record> {
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
            let location = sources::locate(&settings, &path)?;
            let path = location.path.clone();
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
                content_fingerprint: fingerprint(&path).ok(),
                title: title.to_owned(),
                ..Record::new(id, location, SpecStatus::InProgress, timestamp())
            };
            self.write(&record)?;
            Ok(record)
        })
    }

    pub fn get(&self, id: &str) -> Result<Record> {
        let path = self.item_path(id)?;
        self.read_record(&path)
    }

    pub fn list(&self) -> Result<Vec<Record>> {
        let mut records = Vec::new();
        if !self.items_dir().exists() {
            return Ok(records);
        }
        for entry in fs::read_dir(self.items_dir())? {
            let path = entry?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                records.extend(self.read_listed(&path)?);
            }
        }
        records.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| b.id.cmp(&a.id))
        });
        Ok(records)
    }

    pub(super) fn read_record(&self, path: &Path) -> Result<Record> {
        let record: Record = serde_json::from_slice(&fs::read(path)?)?;
        if record.schema_version != SCHEMA_VERSION {
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

    pub(super) fn write(&self, record: &Record) -> Result<()> {
        if record.schema_version != SCHEMA_VERSION {
            return Err("Unsupported metadata schema; refusing to write".into());
        }
        let mut bytes = serde_json::to_vec_pretty(record)?;
        bytes.push(b'\n');
        self.atomic_file(&self.item_path(&record.id)?, &bytes)
    }

    pub fn update(&self, id: &str, change: Change) -> Result<Record> {
        self.locked(|| {
            let mut record = self.get(id)?;
            let settings = self.settings()?;
            let linked = matches!(change, Change::Jira { .. });
            if let Change::Finish {
                spec: Some(path), ..
            } = &change
            {
                self.rebind(&mut record, path.clone(), &settings)?;
            }
            record.apply(change, settings.jira)?;
            if linked {
                self.name_after_ticket(&mut record, &settings)?;
            }
            record.updated_at = timestamp();
            self.write(&record)?;
            Ok(record)
        })
    }

    /// Removes the record of a new spec whose session never opened a tab. Nothing else
    /// refers to it yet, and its spec file was never written.
    pub fn discard_unstarted(&self, id: &str) -> Result<()> {
        self.locked(|| {
            let record = self.get(id)?;
            if record.spec_path.symlink_metadata().is_ok() {
                return Err("Spec file exists; archive the item instead".into());
            }
            fs::remove_file(self.item_path(id)?)?;
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
}
