use super::{App, Prompt};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

pub(super) fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(10),
            Constraint::Length(2),
        ])
        .split(frame.area());
    let rows: Vec<ListItem> = app
        .records
        .iter()
        .map(|record| {
            ListItem::new(format!(
                "{:<7} {:<7} {:<8} {:<6} {}",
                short(&record.spec),
                short(&record.jira.status),
                short(&record.implementation.status),
                short(&record.pr.status),
                record.display_title(),
            ))
        })
        .collect();
    let list = List::new(rows)
        .block(
            Block::default()
                .title(format!(" Inbox · {} items ", app.records.len()))
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let mut state = ListState::default().with_selected(app.current().map(|_| app.selected));
    frame.render_stateful_widget(list, areas[0], &mut state);

    let detail = if let Some(record) = app.current() {
        format!(
            "ID: {}\nRepo: {}\nSpec: {}\nJira: {}  Branch: {}\nSession: {}  Model: {}\nPR: {}\nNext: {}\nError: {}",
            record.id,
            record
                .repo
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "—".into()),
            record.spec_path.display(),
            record.jira.key.as_deref().unwrap_or("—"),
            record.implementation.branch.as_deref().unwrap_or("—"),
            record
                .launch
                .as_ref()
                .map(|launch| launch.status.as_str())
                .unwrap_or("—"),
            record
                .launch
                .as_ref()
                .map(|launch| launch.model.as_str())
                .unwrap_or("—"),
            record.pr.url.as_deref().unwrap_or("—"),
            record.next_actions().join(", "),
            record
                .launch
                .as_ref()
                .and_then(|launch| launch.error.as_deref())
                .unwrap_or("—"),
        )
    } else {
        "No items yet. Press n to start a spec.".into()
    };
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
    } else {
        frame.render_widget(
            Paragraph::new(detail)
                .block(Block::default().title(" Selected ").borders(Borders::ALL)),
            areas[1],
        );
    }
    frame.render_widget(Paragraph::new(footer_text(app)), areas[2]);
}

const COMMANDS: &str =
    "n new · a local · e edit · f finish · t title · J Jira · i dev · p PR · d delete · q quit";

pub(super) fn footer_text(app: &App) -> String {
    if let Some(selected) = app.choice_selected {
        format!(
            "{} selected · j/k choose · Enter continue · Esc cancel",
            app.choices[selected].label()
        )
    } else if matches!(app.prompt.as_ref(), Some(Prompt::Delete { .. })) {
        "Enter delete this item · Esc cancel".into()
    } else if let Some(prompt) = &app.prompt {
        format!(
            "{}: {}█    Enter save · Esc cancel",
            prompt.label(),
            app.input
        )
    } else if !app.message.is_empty() {
        format!("{}\n{COMMANDS}", app.message)
    } else {
        COMMANDS.into()
    }
}

fn short(value: &str) -> &str {
    match value {
        "in_progress" => "working",
        "draft_pr" => "draft",
        other => other,
    }
}
