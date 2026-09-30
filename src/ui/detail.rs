use super::{App, Prompt, Screen, detail_actions, display_implementation_stage};
use crate::store::Record;
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};
use std::fs;

pub(super) fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let Some(record) = app.current().cloned() else {
        return;
    };
    if app.screen == Screen::Reader {
        draw_reader(frame, app, &record);
    } else {
        draw_detail(frame, app, &record);
    }
}

fn draw_detail(frame: &mut ratatui::Frame, app: &mut App, record: &Record) {
    let body = frame.area().inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let compact = body.width < 72;
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(8),
            Constraint::Length(if compact { 5 } else { 4 }),
        ])
        .split(body);
    let title = Line::from(vec![
        Span::styled("← Inbox  /  ", Style::default().fg(Color::Gray)),
        Span::styled(
            fit_label(
                record.display_title(),
                body.width.saturating_sub(13) as usize,
            ),
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(title), areas[0]);
    if !compact {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(38),
                Constraint::Length(2),
                Constraint::Length(32),
            ])
            .split(areas[1]);
        draw_spec(frame, record, columns[0]);
        draw_progress(frame, record, columns[2]);
    } else {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(4), Constraint::Length(9)])
            .split(areas[1]);
        draw_spec(frame, record, rows[0]);
        draw_progress(frame, record, rows[1]);
    }
    draw_actions(frame, app, record, areas[2]);
}

fn draw_spec(frame: &mut ratatui::Frame, record: &Record, area: Rect) {
    let text = spec_text(record);
    let preview = text
        .lines()
        .take(area.height.saturating_sub(2) as usize)
        .collect::<Vec<_>>()
        .join("\n");
    frame.render_widget(
        Paragraph::new(preview)
            .block(Block::default().title("SPEC"))
            .wrap(Wrap { trim: false }),
        Rect::new(area.x, area.y, area.width.min(86), area.height),
    );
}

fn draw_progress(frame: &mut ratatui::Frame, record: &Record, area: Rect) {
    let spec = milestone("SPEC", &record.spec);
    let jira = milestone("JIRA", &record.jira.status);
    let dev = milestone("DEV", display_implementation_stage(record));
    let pr = milestone("PR", record.pr_stage());
    let branch = Style::default().fg(Color::DarkGray);
    let context = Style::default().fg(Color::Gray);
    let lines = vec![
        Line::from("PROGRESS").style(Style::default().fg(Color::Gray)),
        Line::from(spec),
        Line::from(vec![
            Span::styled("│  ", branch),
            Span::styled(
                fit_label(&spec_context(record), area.width.saturating_sub(3) as usize),
                context,
            ),
        ]),
        Line::from(vec![Span::styled("└─ ", branch), jira]),
        Line::from(vec![
            Span::styled("   │  ", branch),
            Span::styled(
                fit_label(&jira_context(record), area.width.saturating_sub(6) as usize),
                context,
            ),
        ]),
        Line::from(vec![Span::styled("   └─ ", branch), dev]),
        Line::from(vec![
            Span::styled("      │  ", branch),
            Span::styled(
                fit_label(&dev_context(record), area.width.saturating_sub(9) as usize),
                context,
            ),
        ]),
        Line::from(vec![Span::styled("      └─ ", branch), pr]),
        Line::from(Span::styled(
            format!(
                "         {}",
                fit_label(&pr_context(record), area.width.saturating_sub(9) as usize)
            ),
            context,
        )),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}

fn spec_context(record: &Record) -> String {
    if let Some(launch) = &record.launch {
        if launch.status == "failed" {
            return "Launch failed".into();
        }
        return format!("{} · {}", launch.workspace, launch.harness);
    }
    if record.spec == "in_progress" {
        "Writing spec".into()
    } else {
        "Spec complete".into()
    }
}

fn jira_context(record: &Record) -> String {
    if let Some(key) = &record.jira.key {
        return format!(
            "{key}{}",
            if record.jira.url.is_some() {
                " ↗"
            } else {
                ""
            }
        );
    }
    if record.jira.status == "ready" {
        "Link ticket next".into()
    } else {
        "After spec".into()
    }
}

fn dev_context(record: &Record) -> String {
    if let Some(branch) = &record.implementation.branch {
        return branch.clone();
    }
    if let Some(agent) = &record.implementation.agent {
        return format!("Agent: {agent}");
    }
    match record.implementation_stage() {
        "ready" => "Ready to start".into(),
        "in_progress" => "In progress".into(),
        _ => "Needs Jira".into(),
    }
}

fn pr_context(record: &Record) -> String {
    if let Some(url) = &record.pr.url {
        let reference = url.trim_end_matches('/').rsplit('/').next().unwrap_or(url);
        return format!("#{reference} ↗");
    }
    match record.pr_stage() {
        "ready" => "Ready to link".into(),
        _ => "Needs implementation".into(),
    }
}

fn fit_label(value: &str, width: usize) -> String {
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

fn draw_actions(frame: &mut ratatui::Frame, app: &mut App, record: &Record, area: Rect) {
    app.action_hitboxes.clear();
    if let Some(prompt) = &app.prompt {
        let content = if matches!(prompt, Prompt::Delete { .. }) {
            format!(
                "Delete {}? Inbox-owned files move to Trash.\nEnter delete · Esc cancel",
                record.display_title()
            )
        } else {
            format!(
                "{}: {}█\nEnter save · Esc cancel",
                prompt.label(),
                app.input
            )
        };
        frame.render_widget(
            Paragraph::new(content).block(Block::default().title(" ACTION ")),
            area,
        );
        return;
    }
    let actions = detail_actions(record);
    let compact = area.width < 72;
    let mut spans = if compact {
        Vec::new()
    } else {
        vec![Span::styled(
            "NEXT MOVE  ",
            Style::default().fg(Color::Gray),
        )]
    };
    let action_y = area.y + u16::from(compact);
    let mut action_x = area.x + if compact { 0 } else { 11 };
    if actions.is_empty() {
        spans.push(Span::styled(
            "All available milestones recorded",
            Style::default().fg(Color::LightGreen),
        ));
    } else {
        for (index, action) in actions.iter().enumerate() {
            let selected = index == app.action_selected;
            let label = format!(" {} {} ", if selected { "▸" } else { "·" }, action.label());
            let width = label.chars().count() as u16;
            app.action_hitboxes
                .push(Rect::new(action_x, action_y, width, 1));
            action_x = action_x.saturating_add(width + 2);
            spans.push(Span::styled(
                label,
                if selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::LightBlue)
                },
            ));
            spans.push(Span::raw("  "));
        }
    }
    let hints = if area.width < 90 {
        "Enter act · r read · e edit · d delete · Esc back"
    } else {
        "Enter act · r full spec · e edit · d delete · Esc list · q quit"
    };
    let hints = if record.jira.url.is_some() {
        format!("{hints} · o Jira")
    } else {
        hints.to_owned()
    };
    let mut lines = Vec::new();
    if compact {
        lines.push(Line::from("NEXT MOVE").style(Style::default().fg(Color::Gray)));
    }
    lines.push(Line::from(spans));
    if compact {
        lines.push(
            Line::from("Enter act · r read · Esc back").style(Style::default().fg(Color::Gray)),
        );
        lines.push(
            Line::from(if record.jira.url.is_some() {
                "e edit · d delete · o open Jira"
            } else {
                "e edit · d delete"
            })
            .style(Style::default().fg(Color::Gray)),
        );
    } else {
        lines.push(Line::from(hints).style(Style::default().fg(Color::Gray)));
    }
    lines.push(Line::from(app.message.as_str()).style(Style::default().fg(Color::LightGreen)));
    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_reader(frame: &mut ratatui::Frame, app: &mut App, record: &Record) {
    let body = frame.area().inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(body);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("← Inbox  /  ", Style::default().fg(Color::Gray)),
            Span::styled(
                fit_label(
                    record.display_title(),
                    body.width.saturating_sub(28) as usize,
                ),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                "  /  FULL SPEC",
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ])),
        areas[0],
    );
    let paragraph = Paragraph::new(spec_text(record)).wrap(Wrap { trim: false });
    let content_lines = paragraph.line_count(areas[1].width);
    let viewport_lines = usize::from(areas[1].height);
    app.reader_max_scroll = if content_lines > viewport_lines {
        content_lines
            .saturating_add(2)
            .saturating_sub(viewport_lines)
            .min(usize::from(u16::MAX)) as u16
    } else {
        0
    };
    app.reader_scroll = app.reader_scroll.min(app.reader_max_scroll);
    frame.render_widget(paragraph.scroll((app.reader_scroll, 0)), areas[1]);
    frame.render_widget(
        Paragraph::new("j/k scroll · Shift+J/K 10 lines").style(Style::default().fg(Color::Gray)),
        areas[2],
    );
}

fn spec_text(record: &Record) -> String {
    match fs::read_to_string(&record.spec_path) {
        Ok(text) if text.trim().is_empty() => "Spec file is empty. Press e to edit it.".into(),
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            "Spec file has not been written yet. It will appear here when the agent saves it."
                .into()
        }
        Err(error) => format!("Cannot read spec: {error}"),
    }
}

fn status_label(status: &str) -> &'static str {
    match status {
        "done" | "created" => "✓ done",
        "in_progress" => "● active",
        "ready" => "→ ready",
        "locked" => "○ locked",
        "draft_pr" | "draft" => "◐ draft",
        "failed" => "✕ failed",
        _ => "○ wait",
    }
}

fn milestone(label: &'static str, status: &str) -> Span<'static> {
    let color = match status {
        "done" | "created" => Color::LightGreen,
        "in_progress" => Color::Cyan,
        "ready" => Color::LightBlue,
        "locked" => Color::DarkGray,
        "draft_pr" | "draft" => Color::Yellow,
        "failed" => Color::Red,
        _ => Color::DarkGray,
    };
    Span::styled(
        format!("{label} {}", status_label(status)),
        Style::default().fg(color),
    )
}
