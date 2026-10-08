use super::{App, ChoicePurpose, Prompt, Screen, detail};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub(super) fn content(area: Rect) -> Rect {
    Rect::new(
        area.x.saturating_add(2),
        area.y.saturating_add(3),
        area.width.saturating_sub(4),
        area.height.saturating_sub(4),
    )
}

/// Bottom navigation row shared by every screen: the last content row, at the content inset.
pub(super) fn footer_area(area: Rect) -> Rect {
    let content = content(area);
    Rect::new(
        content.x,
        content.bottom().saturating_sub(1),
        content.width,
        1.min(content.height),
    )
}

/// Draws shortcut hints with the main list's placement and unstyled terminal foreground.
pub(super) fn draw_footer(frame: &mut ratatui::Frame, hints: &str) {
    frame.render_widget(Paragraph::new(hints), footer_area(frame.area()));
}

pub(super) fn draw(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let header = Rect::new(
        area.x.saturating_add(2),
        area.y.saturating_add(1),
        area.width.saturating_sub(4),
        1.min(area.height.saturating_sub(1)),
    );
    let labels = crumbs(app);
    let labels = fit_crumbs(&labels, header.width.saturating_sub(8) as usize);
    let accent = Style::default()
        .fg(Color::LightCyan)
        .add_modifier(Modifier::BOLD);
    let muted = Style::default().fg(Color::Gray);
    let mut spans = vec![Span::styled(
        "Inbox",
        if labels.is_empty() { accent } else { muted },
    )];
    let count = labels.len();
    for (index, label) in labels.into_iter().enumerate() {
        spans.push(Span::styled(" / ", muted));
        spans.push(Span::styled(
            label,
            if index + 1 == count { accent } else { muted },
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), header);
}

fn crumbs(app: &App) -> Vec<String> {
    match app.screen {
        Screen::Settings => return vec!["Settings".into()],
        Screen::ScanResult => return vec!["Settings".into(), "Scan results".into()],
        _ => {}
    }
    if app.choice_selected.is_some() {
        return match app.choice_purpose {
            ChoicePurpose::NewSpec => vec!["New spec".into()],
            ChoicePurpose::Refine { .. } => {
                let mut path = record_crumbs(app);
                path.push("Refine".into());
                path
            }
        };
    }
    if matches!(
        app.prompt,
        Some(
            Prompt::LaunchWorkspace { .. }
                | Prompt::LaunchRepo { .. }
                | Prompt::LaunchSpec { .. }
                | Prompt::LaunchTopic { .. }
        )
    ) {
        return vec!["New spec".into()];
    }
    if matches!(app.screen, Screen::Detail | Screen::Reader) {
        let mut path = record_crumbs(app);
        if app.screen == Screen::Reader {
            path.push("FULL SPEC".into());
        }
        return path;
    }
    Vec::new()
}

fn record_crumbs(app: &App) -> Vec<String> {
    let Some(record) = app.current() else {
        return Vec::new();
    };
    let Some(index) = app.tree.rows.iter().position(|row| {
        row.record_index
            .is_some_and(|index| app.records[index].id == record.id)
    }) else {
        return vec![record.display_title().into()];
    };
    let mut depth = app.tree.rows[index].depth;
    let mut ancestors = Vec::new();
    for row in app.tree.rows[..index].iter().rev() {
        if row.depth < depth && row.is_folder {
            ancestors.push(row.label.clone());
            depth = row.depth;
        }
    }
    ancestors.reverse();
    ancestors.push(record.display_title().into());
    ancestors
}

fn fit_crumbs(labels: &[String], budget: usize) -> Vec<String> {
    if labels.is_empty() || budget == 0 {
        return Vec::new();
    }
    let total = labels.iter().map(|s| s.chars().count()).sum::<usize>() + (labels.len() - 1) * 3;
    if total <= budget {
        return labels.to_vec();
    }
    if labels.len() == 1 {
        return vec![detail::fit_label(&labels[0], budget)];
    }
    let last = labels.last().unwrap();
    let previous = &labels[labels.len() - 2];
    let ellipsis = labels.len() > 2 && budget >= last.chars().count() + 9;
    let prefix_width = if ellipsis { 4 } else { 0 };
    let last_width = last
        .chars()
        .count()
        .min(budget.saturating_sub(prefix_width + 4).max(budget / 2));
    let previous_width = budget.saturating_sub(prefix_width + last_width + 3);
    let mut result = Vec::new();
    if ellipsis {
        result.push("…".into());
    }
    if previous_width > 0 {
        result.push(detail::fit_label(previous, previous_width));
    }
    result.push(detail::fit_label(last, last_width));
    result
}
