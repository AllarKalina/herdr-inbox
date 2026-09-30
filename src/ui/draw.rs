use super::{App, Prompt, Screen, detail, display_implementation_stage};
use ratatui::layout::{Constraint, Direction, Layout, Margin};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table, TableState,
};

pub(super) fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    if app.screen != Screen::List {
        detail::draw(frame, app);
        return;
    }
    let has_panel = app.prompt.is_some() || app.choice_selected.is_some();
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(if has_panel { 8 } else { 0 }),
            Constraint::Length(2),
        ])
        .split(frame.area());
    let compact = frame.area().width < 64;
    let rows: Vec<Row> = app
        .records
        .iter()
        .enumerate()
        .map(|(index, record)| {
            let selected = index == app.selected;
            let statuses = [
                record.spec.as_str(),
                record.jira.status.as_str(),
                display_implementation_stage(record),
                record.pr_stage(),
            ];
            if compact {
                let mut spans = Vec::new();
                for (index, status) in statuses.iter().enumerate() {
                    if index > 0 {
                        spans.push(Span::raw(" "));
                    }
                    let (label, color) = status_display(status);
                    spans.push(Span::styled(
                        label.chars().next().unwrap_or('?').to_string(),
                        if selected {
                            Style::default()
                        } else {
                            Style::default().fg(color)
                        },
                    ));
                }
                Row::new([
                    Cell::from(record.display_title().to_owned()),
                    Cell::from(Line::from(spans)),
                ])
            } else {
                Row::new([
                    Cell::from(record.display_title().to_owned()),
                    status_cell(statuses[0], selected),
                    status_cell(statuses[1], selected),
                    status_cell(statuses[2], selected),
                    status_cell(statuses[3], selected),
                ])
            }
        })
        .collect();
    let widths = if compact {
        vec![Constraint::Fill(1), Constraint::Length(7)]
    } else {
        vec![
            Constraint::Fill(1),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
        ]
    };
    let header = if compact {
        Row::new([Cell::from(""), Cell::from("S J D P")])
    } else {
        Row::new([
            Cell::from(""),
            Cell::from("Spec"),
            Cell::from("Jira"),
            Cell::from("Dev"),
            Cell::from("PR"),
        ])
    };
    let list = Table::new(rows, widths)
        .header(
            header.style(
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .column_spacing(if compact { 1 } else { 2 })
        .row_highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let mut state = TableState::default()
        .with_offset(app.list_offset)
        .with_selected(app.current().map(|_| app.selected));
    let list_area = areas[0].inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    frame.render_stateful_widget(list, list_area, &mut state);
    app.list_offset = state.offset();
    if app.records.is_empty() {
        let hint = ratatui::layout::Rect::new(
            list_area.x,
            list_area.y.saturating_add(2),
            list_area.width,
            list_area.height.saturating_sub(2),
        );
        frame.render_widget(
            Paragraph::new("No specs yet. Press n to start a spec session.")
                .style(Style::default().fg(Color::Gray)),
            hint,
        );
    }

    if let Some(Prompt::Delete { id }) = &app.prompt {
        let detail = if let Some(record) = app.records.iter().find(|record| &record.id == id) {
            format!(
                "DELETE ITEM\n{}\nID: {}\nSpec: {}\n{}\nOpen agent tabs are not closed.",
                record.display_title(),
                record.id,
                record.spec_path.display(),
                if app.store.manages_spec(record) {
                    "Inbox-owned spec moves to Trash."
                } else {
                    "Linked spec stays in place."
                },
            )
        } else {
            "Item disappeared; press Esc to cancel.".into()
        };
        frame.render_widget(
            Paragraph::new(detail).block(
                Block::default()
                    .title(" Confirm deletion ")
                    .borders(Borders::ALL),
            ),
            areas[1],
        );
    } else if let Some(selected) = app.choice_selected {
        let choices = app
            .choices
            .iter()
            .map(|profile| ListItem::new(profile.label()))
            .collect::<Vec<_>>();
        let list = List::new(choices)
            .block(
                Block::default()
                    .title(" Choose client ")
                    .borders(Borders::ALL),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );
        let mut state = ListState::default().with_selected(Some(selected));
        frame.render_stateful_widget(list, areas[1], &mut state);
    } else if let Some(prompt) = &app.prompt {
        frame.render_widget(
            Paragraph::new(format!("{}: {}█", prompt.label(), app.input))
                .block(Block::default().title(" Action ").borders(Borders::ALL)),
            areas[1],
        );
    }
    frame.render_widget(
        Paragraph::new(footer_text(app)),
        areas[2].inner(Margin {
            horizontal: 1,
            vertical: 0,
        }),
    );
}

const COMMANDS: &str = "Enter open · n new · d delete";

pub(super) fn footer_text(app: &App) -> String {
    if app.choice_selected.is_some() {
        "j/k choose · Enter continue · Esc cancel".into()
    } else if matches!(app.prompt.as_ref(), Some(Prompt::Delete { .. })) {
        "Enter delete this item · Esc cancel".into()
    } else if app.prompt.is_some() {
        "Enter save · Esc cancel".into()
    } else if !app.message.is_empty() {
        format!("{}\n{COMMANDS}", app.message)
    } else {
        COMMANDS.into()
    }
}

fn status_display(status: &str) -> (&'static str, Color) {
    match status {
        "waiting" => ("○ wait", Color::Gray),
        "locked" => ("○ locked", Color::DarkGray),
        "ready" => ("→ ready", Color::LightBlue),
        "in_progress" => ("● active", Color::Cyan),
        "done" | "created" => ("✓ done", Color::LightGreen),
        "draft_pr" | "draft" => ("◐ draft", Color::Yellow),
        "failed" => ("✕ failed", Color::Red),
        _ => ("? check", Color::Yellow),
    }
}

fn status_cell(status: &str, selected: bool) -> Cell<'static> {
    let (label, color) = status_display(status);
    let cell = Cell::from(label);
    if selected {
        cell
    } else {
        cell.style(Style::default().fg(color))
    }
}
