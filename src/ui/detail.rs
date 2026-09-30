use super::{App, Prompt, Screen, detail_actions};
use crate::store::Record;
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::fs;

pub(super) fn draw(frame: &mut ratatui::Frame, app: &App) {
    let Some(record) = app.current() else { return };
    if app.screen == Screen::Reader {
        draw_reader(frame, app, record);
    } else {
        draw_detail(frame, app, record);
    }
}

fn draw_detail(frame: &mut ratatui::Frame, app: &App, record: &Record) {
    let body = frame.area().inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(8),
            Constraint::Length(7),
            Constraint::Length(4),
        ])
        .split(body);
    let title = Line::from(vec![
        Span::styled("← Inbox  /  ", Style::default().fg(Color::Gray)),
        Span::styled(
            record.display_title().to_owned(),
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(title), areas[0]);
    draw_spec(frame, record, areas[1]);
    draw_quest(frame, record, areas[2]);
    draw_actions(frame, app, record, areas[3]);
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
            .block(Block::default().title(" SPEC BRIEF  ·  v read  ·  e edit "))
            .wrap(Wrap { trim: false }),
        Rect::new(area.x, area.y, area.width.min(86), area.height).inner(Margin {
            horizontal: 1,
            vertical: 0,
        }),
    );
}

fn draw_quest(frame: &mut ratatui::Frame, record: &Record, area: Rect) {
    let spec = milestone("SPEC", &record.spec);
    let jira = milestone("JIRA", &record.jira.status);
    let dev = milestone("DEV", &record.implementation.status);
    let pr = milestone("PR", &record.pr.status);
    let lines = vec![
        Line::from("QUEST PATH").style(Style::default().fg(Color::Gray)),
        Line::from(vec![
            Span::raw("  "),
            spec,
            Span::styled(" ━━━┳━━━ ", Style::default().fg(Color::DarkGray)),
            jira,
        ]),
        Line::from(vec![
            Span::styled("             ┗━━━ ", Style::default().fg(Color::DarkGray)),
            dev,
            Span::styled(" ━━━ ", Style::default().fg(Color::DarkGray)),
            pr,
        ]),
        Line::from("Jira and implementation can advance in either order.")
            .style(Style::default().fg(Color::DarkGray)),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::TOP)),
        area,
    );
}

fn draw_actions(frame: &mut ratatui::Frame, app: &App, record: &Record, area: Rect) {
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
    let mut spans = vec![Span::styled(
        "NEXT MOVE  ",
        Style::default().fg(Color::Gray),
    )];
    if actions.is_empty() {
        spans.push(Span::styled(
            "All available milestones recorded",
            Style::default().fg(Color::LightGreen),
        ));
    } else {
        for (index, action) in actions.iter().enumerate() {
            let selected = index == app.action_selected;
            spans.push(Span::styled(
                format!(" {} {} ", if selected { "▸" } else { "·" }, action.label()),
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
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(spans),
            Line::from(
                "Tab select · Enter act · v full spec · e edit · d delete · Esc list · q quit",
            )
            .style(Style::default().fg(Color::Gray)),
            Line::from(app.message.as_str()).style(Style::default().fg(Color::LightGreen)),
        ]),
        area,
    );
}

fn draw_reader(frame: &mut ratatui::Frame, app: &App, record: &Record) {
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
        Paragraph::new(format!("← {}  /  FULL SPEC", record.display_title())).style(
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        ),
        areas[0],
    );
    frame.render_widget(
        Paragraph::new(spec_text(record))
            .wrap(Wrap { trim: false })
            .scroll((app.reader_scroll, 0)),
        areas[1],
    );
    frame.render_widget(
        Paragraph::new("j/k scroll · PgUp/PgDn page · g top · e edit · Esc detail · q quit")
            .style(Style::default().fg(Color::Gray)),
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
        "draft_pr" | "draft" => Color::Yellow,
        "failed" => Color::Red,
        _ => Color::DarkGray,
    };
    Span::styled(
        format!("{label} {}", status_label(status)),
        Style::default().fg(color),
    )
}
