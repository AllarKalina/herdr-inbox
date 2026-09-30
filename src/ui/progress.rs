use super::{App, Prompt, detail::fit_label, milestone::Milestone};
use crate::store::Record;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

const NODE_COLUMN: u16 = 7;
const CONTENT_COLUMN: u16 = 9;

pub(super) fn draw(frame: &mut ratatui::Frame, app: &mut App, record: &Record, area: Rect) {
    app.milestone_hitboxes.clear();
    app.action_hitboxes.clear();
    let muted = Style::default().fg(Color::Gray);
    frame.render_widget(
        Paragraph::new("PROGRESS").style(muted),
        Rect::new(area.x, area.y, area.width, 1),
    );
    let content_width = area.width.saturating_sub(CONTENT_COLUMN);
    let selected = app.milestone_selected;
    let actions_height = if app.prompt.is_some() {
        prompt_lines(app, record, content_width).len() as u16
    } else {
        app.actions().len() as u16
    };
    let guidance_height = if app.prompt.is_none() && app.actions().is_empty() {
        Paragraph::new(selected.guidance(record))
            .wrap(Wrap { trim: false })
            .line_count(content_width)
            .clamp(1, 2) as u16
    } else {
        1
    };
    let required = 9 + actions_height + guidance_height.saturating_sub(1);
    let spacer = u16::from(area.height >= required + 3);
    let mut y = area.y + 1;
    for (index, stage) in Milestone::ALL.iter().enumerate() {
        if y >= area.bottom() {
            break;
        }
        let (node, word, color) = appearance(stage.status(record));
        let is_selected = *stage == selected;
        let label_style = if is_selected {
            selection_style()
        } else {
            Style::default().fg(color)
        };
        let label = Line::from(vec![
            Span::styled(format!("{:<4}", stage.label()), label_style),
            Span::raw("   "),
            Span::styled(format!("{node} {word}"), Style::default().fg(color)),
        ]);
        let hitbox = Rect::new(area.x, y, area.width, 1);
        frame.render_widget(Paragraph::new(label), hitbox);
        app.milestone_hitboxes.push((*stage, hitbox));
        y += 1;
        let context_height = if is_selected { guidance_height } else { 1 };
        let context = if is_selected && app.actions().is_empty() && app.prompt.is_none() {
            stage.guidance(record).to_owned()
        } else {
            fit_label(&stage.context(record), content_width as usize)
        };
        frame.render_widget(
            Paragraph::new(context)
                .style(muted)
                .wrap(Wrap { trim: false }),
            Rect::new(
                area.x + CONTENT_COLUMN,
                y,
                content_width,
                context_height.min(area.bottom().saturating_sub(y)),
            ),
        );
        let block_start = y;
        y += context_height;
        if is_selected {
            let available = Rect::new(
                area.x + CONTENT_COLUMN,
                y,
                content_width,
                area.bottom().saturating_sub(y),
            );
            draw_actions(frame, app, record, available);
            y += actions_height;
        }
        if index < 3 {
            y += spacer;
            let connector_height = y.min(area.bottom()).saturating_sub(block_start);
            frame.render_widget(
                Paragraph::new(vec![Line::from("│"); connector_height as usize])
                    .style(Style::default().fg(Color::DarkGray)),
                Rect::new(area.x + NODE_COLUMN, block_start, 1, connector_height),
            );
        }
    }
}

fn draw_actions(frame: &mut ratatui::Frame, app: &mut App, record: &Record, area: Rect) {
    if app.prompt.is_some() {
        frame.render_widget(Paragraph::new(prompt_lines(app, record, area.width)), area);
        return;
    }
    for (index, action) in app.actions().iter().enumerate() {
        let y = area.y + index as u16;
        if y >= area.bottom() {
            break;
        }
        let selected = index == app.action_selected;
        let label = format!(" {} {} ", if selected { "▸" } else { "·" }, action.label());
        let width = (label.chars().count() as u16).min(area.width);
        let style = if selected {
            selection_style()
        } else {
            Style::default().fg(Color::LightBlue)
        };
        let hitbox = Rect::new(area.x, y, width, 1);
        frame.render_widget(Paragraph::new(label).style(style), hitbox);
        app.action_hitboxes.push(hitbox);
    }
}

fn prompt_lines(app: &App, record: &Record, width: u16) -> Vec<Line<'static>> {
    let Some(prompt) = &app.prompt else {
        return Vec::new();
    };
    let width = usize::from(width);
    let lines = if matches!(prompt, Prompt::Delete { .. }) {
        vec![
            format!(
                "Delete {}?",
                fit_label(record.display_title(), width.saturating_sub(8))
            ),
            if app.store.manages_spec(record) {
                "Files move to local Trash.".into()
            } else {
                "Record moves to Trash.".into()
            },
            if app.store.manages_spec(record) {
                "Enter delete · Esc cancel".into()
            } else {
                "Linked spec stays in place.".into()
            },
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
        lines.push(Line::from("Enter delete · Esc cancel"));
    }
    lines
}

fn selection_style() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
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
