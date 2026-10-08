use super::{App, Field, Result};
use crate::store::SpecSource;
use crate::ui::picker::{self, Kind};
use std::path::PathBuf;

pub(super) fn choose(app: &mut App, field: Field) -> Result<()> {
    let initial = initial_path(app, &field);
    let kind = match field {
        Field::AddSource | Field::Relocate(_) => Kind::Folder,
        _ => Kind::Context,
    };
    let Some(path) = picker::choose(kind, initial.as_deref())? else {
        return Ok(());
    };
    match field {
        Field::AddSource => {
            app.settings.draft.sources.push(SpecSource::new(path)?);
        }
        Field::AddContext | Field::Context(_) => {
            let metadata = std::fs::metadata(&path)?;
            if !metadata.is_file() && !metadata.is_dir() {
                return Err("Context must be an existing file or folder".into());
            }
            if let Field::Context(index) = field {
                app.settings.draft.context_paths[index] = path;
            } else if !app.settings.draft.context_paths.contains(&path) {
                app.settings.draft.context_paths.push(path);
            }
        }
        Field::Relocate(index) => {
            let target = SpecSource::new(path)?.path;
            app.settings
                .begin(Field::ConfirmRelocate(index, target), String::new());
            app.message =
                "Confirm moving the SAME source; add unrelated folders separately.".into();
            return Ok(());
        }
        _ => return Err("This setting does not select a path".into()),
    }
    app.message.clear();
    app.settings.edit = None;
    app.settings.input.clear();
    app.settings.error = false;
    Ok(())
}

fn initial_path(app: &App, field: &Field) -> Option<PathBuf> {
    match field {
        Field::Context(index) => Some(app.settings.draft.context_paths[*index].clone()),
        Field::Relocate(index) => Some(app.settings.draft.sources[*index].path.clone()),
        _ => None,
    }
}

pub(super) fn type_path(app: &mut App) {
    let selected = app.settings.selected;
    let sources = app.settings.draft.sources.len() * 4;
    let contexts = app.settings.draft.context_paths.len();
    let field = if selected < sources {
        if !selected.is_multiple_of(4) {
            return;
        }
        Field::Relocate(selected / 4)
    } else if selected < sources + contexts {
        Field::Context(selected - sources)
    } else {
        match selected - sources - contexts {
            0 => Field::AddSource,
            1 => Field::AddContext,
            _ => return,
        }
    };
    let initial = initial_path(app, &field)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    app.settings.begin(field, initial);
    app.message.clear();
}
