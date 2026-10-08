//! Terminal colour roles shared by every screen, named after their job in DESIGN.md.
//! Ratatui emits roles, never hex values: the host terminal's palette decides the result.

use ratatui::style::{Color, Modifier, Style};

/// The selected row in a list: Inbox, client choice, Settings, Archive.
pub(super) fn selection() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

/// The current breadcrumb segment.
pub(super) fn accent() -> Style {
    Style::default()
        .fg(Color::LightCyan)
        .add_modifier(Modifier::BOLD)
}

/// The action Enter will run in the detail view.
pub(super) fn chosen_action() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
}

/// An available action that is not the chosen one.
pub(super) fn action() -> Style {
    Style::default().fg(Color::LightBlue)
}

/// Secondary labels, headings, ancestors in the breadcrumb, and empty states.
pub(super) fn muted() -> Style {
    Style::default().fg(Color::Gray)
}

/// Structure that should recede: tree guides and the progress spine.
pub(super) fn faint() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub(super) fn success() -> Style {
    Style::default().fg(Color::LightGreen)
}

pub(super) fn warning() -> Style {
    Style::default().fg(Color::Yellow)
}

pub(super) fn error() -> Style {
    Style::default().fg(Color::Red)
}
