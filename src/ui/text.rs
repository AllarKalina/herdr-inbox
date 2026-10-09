//! Fitting titles and paths into terminal columns.

use std::path::Path;

/// Keeps the start of a label, ending with an ellipsis when it cannot fit.
pub(super) fn fit_label(value: &str, width: usize) -> String {
    let count = value.chars().count();
    if count <= width {
        return value.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let mut result: String = value.chars().take(width - 1).collect();
    result.push('…');
    result
}

/// Shortens the home directory to `~` for display.
pub(super) fn tilde(path: &Path) -> String {
    std::env::var_os("HOME")
        .and_then(|home| path.strip_prefix(home).ok())
        .map_or_else(
            || path.display().to_string(),
            |rest| format!("~/{}", rest.display()),
        )
}

/// Keeps the end of a path, its most specific part, when it cannot fit.
pub(super) fn fit_tail(value: &str, width: usize) -> String {
    let count = value.chars().count();
    if count <= width {
        return value.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let tail: String = value.chars().skip(count - (width - 1)).collect();
    format!("…{tail}")
}

/// Keeps the end of a tree prefix so the nearest branch guides stay visible.
pub(super) fn fit_prefix(prefix: &str, available: usize) -> String {
    if prefix.chars().count() <= available {
        return prefix.into();
    }
    if available < 3 {
        return String::new();
    }
    let suffix = prefix
        .chars()
        .rev()
        .take(available - 1)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("…{suffix}")
}
