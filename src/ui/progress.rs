use super::{App, Prompt, detail::fit_label, milestone::Milestone};
use crate::store::Record;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::canvas::{Canvas, Circle};
use ratatui::widgets::{Paragraph, Wrap};

const NODE_COLUMN: u16 = 7;
const CONTENT_COLUMN: u16 = 11;

pub(super) fn draw(frame: &mut ratatui::Frame, app: &mut App, record: &Record, area: Rect) {
    app.milestone_hitboxes.clear();
    app.action_hitboxes.clear();
    let muted = Style::default().fg(Color::Gray);
    frame.render_widget(
        Paragraph::new("PROGRESS").style(muted),
        Rect::new(area.x, area.y, area.width, 1),
    );
    let selected = app.milestone_selected;
    // Geometry depends only on the viewport, never on selection or prompt content.
    let compact = area.height < 26;
    let stride = if compact {
        area.height.saturating_sub(4).saturating_div(3).max(1)
    } else {
        6
    };
    for (index, stage) in Milestone::ALL.iter().enumerate() {
        let y = area.y + 2 + index as u16 * stride;
        if y >= area.bottom() {
            break;
        }
        let (node, word, color) = appearance(stage.status(record));
        let is_selected = *stage == selected;
        let label = Line::from(vec![
            Span::styled(format!("{:<4}", stage.label()), Style::default().fg(color)),
            Span::raw("   "),
            Span::styled(format!("{node}   {word}"), Style::default().fg(color)),
        ]);
        let hitbox = Rect::new(
            area.x,
            y,
            if compact {
                18.min(area.width)
            } else {
                area.width
            },
            1,
        );
        frame.render_widget(Paragraph::new(label), hitbox);
        app.milestone_hitboxes.push((*stage, hitbox));
        let feedback = app
            .feedback
            .as_ref()
            .filter(|feedback| feedback.milestone == *stage);
        if !compact {
            let content_width = area.width.saturating_sub(CONTENT_COLUMN);
            let context = stage.context(record);
            frame.render_widget(
                Paragraph::new(fit_label(
                    feedback.map_or(context.as_str(), |feedback| feedback.text(false)),
                    content_width as usize,
                ))
                .style(if feedback.is_some() {
                    Style::default().fg(Color::LightGreen)
                } else {
                    muted
                }),
                Rect::new(area.x + CONTENT_COLUMN, y + 1, content_width, 1),
            );
            if is_selected {
                draw_actions(
                    frame,
                    app,
                    record,
                    Rect::new(area.x + CONTENT_COLUMN, y + 2, content_width, 4),
                    1,
                );
            }
        } else if let Some(feedback) = feedback {
            frame.render_widget(
                Paragraph::new(feedback.text(true)).style(Style::default().fg(Color::LightGreen)),
                Rect::new(
                    area.x + CONTENT_COLUMN,
                    y + 1,
                    7.min(area.width.saturating_sub(CONTENT_COLUMN)),
                    1,
                ),
            );
        }
        if index < 3 {
            let connector_height = stride
                .saturating_sub(1)
                .min(area.bottom().saturating_sub(y + 1));
            frame.render_widget(
                Paragraph::new(vec![Line::from("│"); connector_height as usize])
                    .style(Style::default().fg(Color::DarkGray)),
                Rect::new(area.x + NODE_COLUMN, y + 1, 1, connector_height),
            );
        }
    }
    if compact {
        let panel_x = (area.x + 20).min(area.right());
        let panel_width = area.right().saturating_sub(panel_x);
        frame.render_widget(
            Paragraph::new(format!("{} ACTIONS", selected.label())).style(muted),
            Rect::new(panel_x, area.y + 1, panel_width, 1),
        );
        let context = if app.actions().is_empty() && app.prompt.is_none() {
            selected.guidance(record).to_owned()
        } else {
            selected.context(record)
        };
        frame.render_widget(
            Paragraph::new(context)
                .style(muted)
                .wrap(Wrap { trim: false }),
            Rect::new(panel_x, area.y + 3, panel_width, 4),
        );
        draw_actions(
            frame,
            app,
            record,
            Rect::new(
                panel_x,
                area.y + 7,
                panel_width,
                area.height.saturating_sub(7),
            ),
            2,
        );
    }
    if let Some((_, selected_area)) = app
        .milestone_hitboxes
        .iter()
        .find(|(stage, _)| *stage == selected)
    {
        // A terminal-native circle, drawn inside space reserved for every stage.
        // Overlay only the focus ring; retain the semantic status at its center.
        frame.render_widget(
            Canvas::default()
                .x_bounds([-1.0, 1.0])
                .y_bounds([-1.0, 1.0])
                .paint(|context| {
                    context.draw(&Circle {
                        x: 0.0,
                        y: 0.0,
                        radius: 0.9,
                        color: Color::Cyan,
                    })
                }),
            Rect::new(area.x + NODE_COLUMN - 2, selected_area.y - 1, 5, 3),
        );
        let (node, _, color) = appearance(selected.status(record));
        frame.render_widget(
            Paragraph::new(node).style(Style::default().fg(color)),
            Rect::new(area.x + NODE_COLUMN, selected_area.y, 1, 1),
        );
    }
}

fn draw_actions(
    frame: &mut ratatui::Frame,
    app: &mut App,
    record: &Record,
    area: Rect,
    slot_height: u16,
) {
    if app.prompt.is_some() {
        frame.render_widget(
            Paragraph::new(prompt_lines(app, record, area.width)).wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    let actions = app.actions();
    if actions.is_empty() && slot_height == 1 {
        frame.render_widget(
            Paragraph::new(app.milestone_selected.guidance(record))
                .style(Style::default().fg(Color::Gray))
                .wrap(Wrap { trim: false }),
            area,
        );
    }
    for (index, action) in actions.iter().enumerate() {
        let y = area.y + index as u16 * slot_height;
        if y >= area.bottom() {
            break;
        }
        let selected = index == app.action_selected;
        let label = format!("{}{}", if selected { "✦ " } else { "  " }, action.label());
        let width = (label.chars().count() as u16).min(area.width);
        let style = if selected {
            selection_style()
        } else {
            Style::default().fg(Color::LightBlue)
        };
        let height = Paragraph::new(label.as_str())
            .wrap(Wrap { trim: false })
            .line_count(width)
            .min(usize::from(slot_height)) as u16;
        let hitbox = Rect::new(
            area.x,
            y,
            width,
            height.min(area.bottom().saturating_sub(y)),
        );
        frame.render_widget(
            Paragraph::new(label)
                .style(style)
                .wrap(Wrap { trim: false }),
            hitbox,
        );
        app.action_hitboxes.push(hitbox);
    }
}

fn prompt_lines(app: &App, record: &Record, width: u16) -> Vec<Line<'static>> {
    let Some(prompt) = &app.prompt else {
        return Vec::new();
    };
    let width = usize::from(width);
    let archive_keys = if width < 25 {
        "Enter archive · Esc"
    } else {
        "Enter archive · Esc cancel"
    };
    let lines = if matches!(prompt, Prompt::Delete { .. }) {
        vec![
            format!(
                "Archive {}?",
                fit_label(record.display_title(), width.saturating_sub(9))
            ),
            if app.store.manages_spec(record) {
                "Files move to local archive.".into()
            } else {
                "Record moves to archive.".into()
            },
            if app.store.manages_spec(record) {
                archive_keys.into()
            } else {
                "Linked spec stays intact.".into()
            },
        ]
    } else if matches!(prompt, Prompt::Settle { .. }) {
        vec![
            "Session ended?".into(),
            "No tabs closed.".into(),
            "Enter settle".into(),
            "Esc cancel".into(),
        ]
    } else {
        let tail: String = app
            .input
            .chars()
            .rev()
            .take(width.saturating_sub(1))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        vec![
            fit_label(prompt.label(), width),
            format!("{tail}█"),
            "Enter save · Esc cancel".into(),
        ]
    };
    let mut lines: Vec<Line<'static>> = lines.into_iter().map(Line::from).collect();
    if matches!(prompt, Prompt::Delete { .. }) && !app.store.manages_spec(record) {
        lines.push(Line::from(archive_keys));
    }
    lines
}

fn selection_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
}

fn appearance(status: &str) -> (&'static str, &'static str, Color) {
    match status {
        "done" | "created" => ("●", "done", Color::LightGreen),
        "in_progress" => ("◉", "active", Color::Cyan),
        "ready" => ("○", "ready", Color::LightBlue),
        "locked" => ("○", "locked", Color::DarkGray),
        "draft_pr" | "draft" => ("◐", "draft", Color::Yellow),
        "failed" => ("✕", "failed", Color::Red),
        _ => ("○", "wait", Color::Gray),
    }
}
