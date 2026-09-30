use super::{App, Prompt, Screen, detail};
use ratatui::layout::{Constraint, Direction, Layout, Margin};
use ratatui::style::{Color, Modifier, Style};
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
    let rows: Vec<Row> = app
        .records
        .iter()
        .enumerate()
        .map(|(index, record)| {
            let selected = index == app.selected;
            Row::new([
                Cell::from(record.display_title().to_owned()),
                status_cell(&record.spec, selected),
                status_cell(&record.jira.status, selected),
                status_cell(&record.implementation.status, selected),
                status_cell(&record.pr.status, selected),
            ])
        })
        .collect();
    let list = Table::new(
        rows,
        [
            Constraint::Fill(1),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new([
            Cell::from(""),
            Cell::from("Spec"),
            Cell::from("Jira"),
            Cell::from("Dev"),
            Cell::from("PR"),
        ])
        .style(
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .column_spacing(2)
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
            Paragraph::new("No specs yet. Press n to start a session, or a to add a local item.")
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

const COMMANDS: &str = "Enter open · n new · a local · e edit · f finish · t title · J Jira · i dev · p PR · d delete · q quit";

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

fn status_cell(status: &str, selected: bool) -> Cell<'static> {
    let (label, color) = match status {
        "waiting" => ("○ wait", Color::Gray),
        "ready" => ("→ ready", Color::LightBlue),
        "in_progress" => ("● active", Color::Cyan),
        "done" | "created" => ("✓ done", Color::LightGreen),
        "draft_pr" | "draft" => ("◐ draft", Color::Yellow),
        "failed" => ("✕ failed", Color::Red),
        _ => ("? check", Color::Yellow),
    };
    let cell = Cell::from(label);
    if selected {
        cell
    } else {
        cell.style(Style::default().fg(color))
    }
}
