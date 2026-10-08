use super::{App, Result};
use crate::store::SpecSource;
use crate::ui::picker;

pub(super) fn choose(app: &App) -> Result<Option<SpecSource>> {
    let initial = app
        .settings
        .config
        .sources
        .first()
        .map(|source| source.path.as_path());
    let Some(path) = picker::choose(initial)? else {
        return Ok(None);
    };
    let selected =
        SpecSource::new(path).map_err(|_| "Could not open that folder. Choose another folder.")?;
    // Reselecting the same physical folder keeps its identity and its CLI-configured filters.
    let existing = app
        .settings
        .config
        .sources
        .iter()
        .find(|source| source.path.canonicalize().ok().as_ref() == Some(&selected.path));
    Ok(Some(existing.cloned().unwrap_or(selected)))
}
