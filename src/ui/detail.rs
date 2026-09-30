use super::{App, Screen, progress};
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
    let mut body = frame.area().inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    // Reclaim the bottom inset on short terminals to keep the progress controls visible.
    if body.height < 20 {
        body.height = body.height.saturating_add(1);
    }
    let compact = body.width < 78;
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if body.height < 24 { 1 } else { 2 }),
            Constraint::Min(8),
            Constraint::Length(if body.height < 20 { 2 } else { 3 }),
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
                Constraint::Min(40),
                Constraint::Length(2),
                Constraint::Length(36),
            ])
            .split(areas[1]);
        draw_spec(frame, record, columns[0]);
        progress::draw(frame, app, record, columns[2]);
    } else {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(if areas[1].height < 18 { 0 } else { 3 }),
                Constraint::Length(15.min(areas[1].height)),
            ])
            .split(areas[1]);
        draw_spec(frame, record, rows[0]);
        progress::draw(frame, app, record, rows[1]);
    }
    frame.render_widget(
        Paragraph::new(vec![
            Line::default(),
            Line::from("j/k stage · Tab action · r read").style(Style::default().fg(Color::Gray)),
            Line::from(app.message.as_str()).style(Style::default().fg(Color::LightGreen)),
        ]),
        areas[2],
    );
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
