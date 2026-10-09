//! The settings file and the only ways to change it.

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

    /// Replaces the settings wholesale. Production code changes them through
    /// `update_settings`; tests use this to set up a known configuration.
    #[cfg(test)]
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
}
