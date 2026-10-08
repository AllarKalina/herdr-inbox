use crate::store::{Result, absolute};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub sources: Vec<SpecSource>,
    pub context_paths: Vec<PathBuf>,
    pub preferred_client: Option<String>,
    pub workspace: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            sources: Vec::new(),
            context_paths: Vec::new(),
            preferred_client: None,
            workspace: "ai-boiler-room".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpecSource {
    pub id: String,
    pub path: PathBuf,
    #[serde(default = "yes")]
    pub recursive: bool,
    #[serde(default = "default_include")]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

fn yes() -> bool {
    true
}
fn default_include() -> Vec<String> {
    vec!["**/*.md".into(), "**/*.markdown".into()]
}

impl SpecSource {
    pub fn new(path: PathBuf) -> Result<Self> {
        let path = absolute(path)?;
        if !path.is_dir() {
            return Err(format!(
                "Spec source is not an available directory: {}",
                path.display()
            )
            .into());
        }
        std::fs::read_dir(&path)?;
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            path,
            recursive: true,
            include: default_include(),
            exclude: Vec::new(),
        })
    }

    pub fn filters(&self) -> Result<(GlobSet, GlobSet)> {
        fn build(patterns: &[String]) -> Result<GlobSet> {
            let mut builder = GlobSetBuilder::new();
            for pattern in patterns {
                builder.add(Glob::new(pattern)?);
            }
            Ok(builder.build()?)
        }
        Ok((build(&self.include)?, build(&self.exclude)?))
    }
}

impl Settings {
    /// Validate syntax and normalize local references. Missing content remains configurable.
    pub fn validate(&mut self) -> Result<()> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "Unsupported settings schema {}; expected {}",
                self.schema_version, SCHEMA_VERSION
            )
            .into());
        }
        if self.workspace.trim().is_empty() {
            return Err("Workspace cannot be empty".into());
        }
        self.workspace = self.workspace.trim().into();
        let mut ids = HashSet::new();
        for source in &mut self.sources {
            Uuid::parse_str(&source.id)?;
            if !ids.insert(source.id.clone()) {
                return Err("Duplicate source ID".into());
            }
            source.path = absolute(source.path.clone())?;
            if source.include.is_empty() {
                return Err("Source must have at least one include filter".into());
            }
            source.filters()?;
        }
        for path in &mut self.context_paths {
            *path = absolute(path.clone())?;
        }
        if self
            .preferred_client
            .as_ref()
            .is_some_and(|s| s.trim().is_empty())
        {
            return Err("Preferred client cannot be empty".into());
        }
        Ok(())
    }

    pub fn validate_context(&self) -> Result<()> {
        for path in &self.context_paths {
            let metadata = std::fs::metadata(path).map_err(|error| {
                format!(
                    "Context unavailable: {} ({error}); correct or remove it in settings",
                    path.display()
                )
            })?;
            if metadata.is_dir() {
                std::fs::read_dir(path)?;
            } else if metadata.is_file() {
                std::fs::File::open(path)?;
            } else {
                return Err(format!("Unsupported context reference: {}", path.display()).into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_and_filters_roundtrip() -> Result<()> {
        let mut settings = Settings::default();
        settings
            .sources
            .push(SpecSource::new(std::env::temp_dir())?);
        let serialized = toml::to_string_pretty(&settings)?;
        let mut restored: Settings = toml::from_str(&serialized)?;
        restored.validate()?;
        let (include, _) = restored.sources[0].filters()?;
        assert!(include.is_match("one.md"));
        assert!(include.is_match("nested/two.markdown"));
        assert!(!include.is_match("code.rs"));
        restored.schema_version = 99;
        assert!(restored.validate().is_err());
        Ok(())
    }
}
