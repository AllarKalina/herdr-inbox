use crate::store::{Result, absolute};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub schema_version: u32,
    pub sources: Vec<SpecSource>,
    pub context_paths: Vec<PathBuf>,
    pub preferred_client: Option<String>,
    /// Whether the Jira stage is part of this computer's workflow.
    pub jira: bool,
    /// The epic or story the last ticket was created under, offered again next time.
    pub jira_parent: Option<String>,
    /// The skill that implements a spec, such as `/team-dev`. Without one, development is
    /// only recorded, never launched.
    pub dev_skill: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            sources: Vec::new(),
            context_paths: Vec::new(),
            preferred_client: None,
            jira: true,
            jira_parent: None,
            dev_skill: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpecSource {
    pub id: String,
    pub path: PathBuf,
    pub recursive: bool,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

fn default_include() -> Vec<String> {
    vec!["**/*.md".into(), "**/*.markdown".into()]
}

impl SpecSource {
    pub fn new(path: PathBuf) -> Result<Self> {
        let path = std::fs::canonicalize(absolute(path)?)?;
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

/// Resolve membership from the physical path; metadata never determines scope.
/// The caller resolves symlinks before calling. New destinations may not exist yet.
pub fn source_location<'a>(
    sources: &'a [SpecSource],
    resolved_path: &Path,
) -> Option<(&'a SpecSource, PathBuf)> {
    if !resolved_path.is_absolute()
        || resolved_path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return None;
    }
    sources
        .iter()
        .filter_map(|source| {
            let root = source.path.canonicalize().ok()?;
            if !root.is_dir() {
                return None;
            }
            let relative = resolved_path.strip_prefix(&root).ok()?.to_path_buf();
            if relative.as_os_str().is_empty()
                || !source.recursive && relative.components().count() > 1
            {
                return None;
            }
            let (include, exclude) = source.filters().ok()?;
            if !include.is_match(&relative)
                || relative
                    .ancestors()
                    .any(|ancestor| !ancestor.as_os_str().is_empty() && exclude.is_match(ancestor))
            {
                return None;
            }
            Some((source, relative, root.components().count()))
        })
        .max_by(|(a, _, depth_a), (b, _, depth_b)| {
            depth_a.cmp(depth_b).then_with(|| b.id.cmp(&a.id))
        })
        .map(|(source, relative, _)| (source, relative))
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
        for (name, value) in [
            ("Jira parent", &self.jira_parent),
            ("Dev skill", &self.dev_skill),
        ] {
            if value.as_ref().is_some_and(|text| text.trim().is_empty()) {
                return Err(format!("{name} cannot be empty; remove it instead").into());
            }
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

    #[test]
    fn persisted_settings_require_current_fields() -> Result<()> {
        for incomplete in ["", "schema_version = 1\n", "schema_version = 0\n"] {
            assert!(toml::from_str::<Settings>(incomplete).is_err());
        }
        let serialized = toml::to_string_pretty(&Settings::default())?;
        let without_schema = serialized.replace("schema_version = 1\n", "");
        assert!(toml::from_str::<Settings>(&without_schema).is_err());
        Ok(())
    }

    #[test]
    fn membership_uses_physical_scope_filters_and_deepest_root_only() -> Result<()> {
        let root = std::env::temp_dir().join(format!("herdr-membership-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("nested/ignored"))?;
        let broad = SpecSource::new(root.clone())?;
        let mut narrow = SpecSource::new(root.join("nested"))?;
        narrow.exclude = vec!["ignored".into()];
        let canonical = root.canonicalize()?;
        let new_path = canonical.join("nested/new.md");
        let sources = vec![broad.clone(), narrow.clone()];
        let (selected, relative) = source_location(&sources, &new_path).unwrap();
        assert_eq!(selected.id, narrow.id);
        assert_eq!(relative, PathBuf::from("new.md"));
        assert!(!new_path.exists());
        assert!(
            source_location(
                std::slice::from_ref(&narrow),
                &canonical.join("nested/ignored/new.md")
            )
            .is_none()
        );
        narrow.recursive = false;
        assert!(
            source_location(
                std::slice::from_ref(&narrow),
                &canonical.join("nested/deeper/new.md")
            )
            .is_none()
        );
        assert!(source_location(&sources, &canonical.join("code.rs")).is_none());
        assert!(source_location(&sources, &canonical.join("../outside.md")).is_none());
        let mut a = broad.clone();
        let mut b = broad;
        a.id = "a".into();
        b.id = "b".into();
        assert_eq!(
            source_location(&[b.clone(), a.clone()], &canonical.join("new.md"))
                .unwrap()
                .0
                .id,
            "a"
        );
        assert_eq!(
            source_location(&[a, b], &canonical.join("new.md"))
                .unwrap()
                .0
                .id,
            "a"
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
