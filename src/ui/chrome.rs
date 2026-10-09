//! What every screen shares: the anchored breadcrumb header, the content area, the notice
//! row and the shortcut line.

use super::notice::{Notice, Tone};
use super::{text, theme};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// The area below the header, inset two columns on each side and one row at the bottom.
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

/// Draws shortcut hints in the unstyled terminal foreground.
pub(super) fn draw_footer(frame: &mut ratatui::Frame, hints: &str) {
    frame.render_widget(Paragraph::new(hints), footer_area(frame.area()));
}

/// Draws the notice on one row, coloured by its tone.
pub(super) fn draw_notice(frame: &mut ratatui::Frame, notice: &Notice, row: Rect) {
    let style = match notice.tone() {
        Tone::Info => Style::default(),
        Tone::Success => theme::success(),
        Tone::Error => theme::error(),
    };
    let fitted = text::fit_label(notice.text(), usize::from(row.width));
    frame.render_widget(Paragraph::new(fitted).style(style), row);
}

/// Draws `Inbox / … / current`: the anchor stays put, and only the last segment is accented.
pub(super) fn draw_header(frame: &mut ratatui::Frame, crumbs: &[String]) {
    let area = frame.area();
    let header = Rect::new(
        area.x.saturating_add(2),
        area.y.saturating_add(1),
        area.width.saturating_sub(4),
        1.min(area.height.saturating_sub(1)),
    );
    let labels = fit_crumbs(crumbs, header.width.saturating_sub(8) as usize);
    let (accent, muted) = (theme::accent(), theme::muted());
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

/// Shortens long ancestry while keeping the current location readable.
fn fit_crumbs(labels: &[String], budget: usize) -> Vec<String> {
    if labels.is_empty() || budget == 0 {
        return Vec::new();
    }
    let total = labels.iter().map(|s| s.chars().count()).sum::<usize>() + (labels.len() - 1) * 3;
    if total <= budget {
        return labels.to_vec();
    }
    if labels.len() == 1 {
        return vec![text::fit_label(&labels[0], budget)];
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
        result.push(text::fit_label(previous, previous_width));
    }
    result.push(text::fit_label(last, last_width));
    result
}
