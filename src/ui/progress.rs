use super::{App, Prompt, detail::fit_label, milestone::Milestone};
use crate::store::Record;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

pub(super) fn draw(frame: &mut ratatui::Frame, app: &mut App, record: &Record, area: Rect) {
    app.milestone_hitboxes.clear();
    app.action_hitboxes.clear();
    let muted = Style::default().fg(Color::Gray);
    frame.render_widget(
        Paragraph::new("PROGRESS").style(muted),
        Rect::new(area.x, area.y, area.width, 1),
    );
    let stride = if area.height >= 21 { 3 } else { 2 };
    for (index, stage) in Milestone::ALL.iter().enumerate() {
        let y = area.y + 1 + index as u16 * stride;
        if y >= area.bottom() {
            break;
        }
        let status = stage.status(record);
        let (node, word, color) = appearance(status);
        let selected = *stage == app.milestone_selected;
        let style = if selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(color)
        };
        let hitbox = Rect::new(area.x, y, area.width, 1);
        let label = format!("{:<7}{node} {word}", stage.label());
        frame.render_widget(Paragraph::new(label).style(style), hitbox);
        app.milestone_hitboxes.push((*stage, hitbox));
        let connector = if index < 3 { "│" } else { " " };
        if y + 1 < area.bottom() {
            let context = Line::from(vec![
                Span::styled(
                    format!("       {connector} "),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    fit_label(
                        &stage.context(record),
                        area.width.saturating_sub(9) as usize,
                    ),
                    muted,
                ),
            ]);
            frame.render_widget(
                Paragraph::new(context),
                Rect::new(area.x, y + 1, area.width, 1),
            );
        }
        if stride == 3 && index < 3 && y + 2 < area.bottom() {
            frame.render_widget(
                Paragraph::new("       │").style(Style::default().fg(Color::DarkGray)),
                Rect::new(area.x, y + 2, area.width, 1),
            );
        }
    }
    let action_y = area.y + 1 + 3 * stride + 3;
    let actions_area = Rect::new(
        area.x,
        action_y.min(area.bottom()),
        area.width,
        area.bottom().saturating_sub(action_y),
    );
    draw_actions(frame, app, record, actions_area);
}

fn draw_actions(frame: &mut ratatui::Frame, app: &mut App, record: &Record, area: Rect) {
    if let Some(prompt) = &app.prompt {
        let text = if matches!(prompt, Prompt::Delete { .. }) {
            format!(
                "Delete {}?\n{}\nEnter delete · Esc cancel",
                fit_label(
                    record.display_title(),
                    area.width.saturating_sub(8) as usize
                ),
                if app.store.manages_spec(record) {
                    "Files move to local Trash."
                } else {
                    "Record moves to Trash; linked spec stays."
                }
            )
        } else {
            format!(
                "{}\n{}█\nEnter save · Esc cancel",
                prompt.label(),
                app.input
            )
        };
        frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), area);
        return;
    }
    let stage = app.milestone_selected;
    let guidance = Paragraph::new(stage.guidance(record))
        .style(Style::default().fg(Color::Gray))
        .wrap(Wrap { trim: false });
    let guidance_height = guidance.line_count(area.width).min(2) as u16;
    frame.render_widget(
        guidance,
        Rect::new(area.x, area.y, area.width, guidance_height.min(area.height)),
    );
    for (index, action) in app.actions().iter().enumerate() {
        let y = area.y + guidance_height + index as u16;
        if y >= area.bottom() {
            break;
        }
        let selected = index == app.action_selected;
        let label = format!(" {} {} ", if selected { "▸" } else { "·" }, action.label());
        let width = (label.chars().count() as u16).min(area.width);
        let style = if selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::LightBlue)
        };
        let hitbox = Rect::new(area.x, y, width, 1);
        frame.render_widget(Paragraph::new(label).style(style), hitbox);
        app.action_hitboxes.push(hitbox);
    }
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
