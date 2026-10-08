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
    let mut selected =
        SpecSource::new(path).map_err(|_| "Could not open that folder. Choose another folder.")?;
    // Reselecting the same physical folder retains its workflow identity.
    if let Some(existing) = app
        .settings
        .config
        .sources
        .iter()
        .find(|source| source.path.canonicalize().ok().as_ref() == Some(&selected.path))
    {
        selected.id.clone_from(&existing.id);
    }
    Ok(Some(selected))
}
