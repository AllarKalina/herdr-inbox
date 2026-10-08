use super::{App, ChoicePurpose, Prompt, Screen, detail};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};

use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

pub(super) fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    super::chrome::draw(frame, app);
    if app.screen == Screen::ScanResult {
        super::scan::draw(frame, app);
        return;
    }
    if app.screen == Screen::Settings {
        super::settings::draw(frame, app);
        return;
    }
    if app.screen != Screen::List {
        if let Some(selected) = app.choice_selected {
            let width = frame.area().width.saturating_sub(4).min(52);
            let height = frame.area().height.saturating_sub(2).min(9);
            let popup = Rect::new(
                frame.area().x + (frame.area().width - width) / 2,
                frame.area().y + (frame.area().height - height) / 2,
                width,
                height,
            );
            draw_client_choices(
                frame,
                app,
                selected,
                Rect::new(popup.x, popup.y, popup.width, 5.min(popup.height)),
            );
            frame.render_widget(
                Paragraph::new(app.message.as_str())
                    .style(Style::default().fg(Color::Red))
                    .wrap(Wrap { trim: false }),
                Rect::new(
                    popup.x + 1,
                    popup.y + 5,
                    popup.width.saturating_sub(2),
                    popup.height.saturating_sub(6),
                ),
            );
            let footer = super::chrome::footer_area(frame.area());
            super::chrome::draw_footer(frame, choice_commands(footer.width));
        } else {
            detail::draw(frame, app);
        }
        return;
    }
    let has_panel = app.prompt.is_some() || app.choice_selected.is_some();
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(if has_panel { 8 } else { 0 }),
            Constraint::Length(if app.message.is_empty() { 1 } else { 2 }),
        ])
        .split(super::chrome::content(frame.area()));
    let list_area = areas[0];
    super::list::draw(frame, app, list_area);
    if app.records.is_empty() {
        let hint = ratatui::layout::Rect::new(
            list_area.x,
            list_area.y.saturating_add(2),
            list_area.width,
            list_area.height.saturating_sub(2),
        );
        frame.render_widget(
            Paragraph::new("No specs yet.").style(Style::default().fg(Color::Gray)),
            hint,
        );
    }

    if let Some(Prompt::Archive { id }) = &app.prompt {
        let detail = if let Some(record) = app.records.iter().find(|record| &record.id == id) {
            format!(
                "ARCHIVE ITEM\n{}\nID: {}\nSpec: {}\n{}\nOpen agent tabs are not closed.",
                record.display_title(),
                record.id,
                record.spec_path.display(),
                "Linked spec stays in place.",
            )
        } else {
            "Item disappeared; press Esc to cancel.".into()
        };
        frame.render_widget(
            Paragraph::new(detail).block(
                Block::default()
                    .title(" Confirm archive ")
                    .borders(Borders::ALL),
            ),
            areas[1],
        );
    } else if let Some(selected) = app.choice_selected {
        draw_client_choices(frame, app, selected, areas[1]);
    } else if let Some(prompt) = &app.prompt {
        frame.render_widget(
            Paragraph::new(format!("{}: {}█", prompt.label(), app.input))
                .block(Block::default().title(" Action ").borders(Borders::ALL)),
            areas[1],
        );
    }
    let footer_area = areas[2];
    frame.render_widget(
        Paragraph::new(footer_text(app, footer_area.width)),
        footer_area,
    );
}

fn draw_client_choices(frame: &mut ratatui::Frame, app: &App, selected: usize, area: Rect) {
    let choices = app
        .choices
        .iter()
        .map(|profile| ListItem::new(profile.label()))
        .collect::<Vec<_>>();
    let title = if matches!(app.choice_purpose, ChoicePurpose::Refine { .. }) {
        " Refine spec · Choose client "
    } else {
        " Choose client "
    };
    let list = List::new(choices)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let mut state = ListState::default().with_selected(Some(selected));
    frame.render_stateful_widget(list, area, &mut state);
}

const COMMANDS: &str = "Enter open · n new · a archive · s settings";
const CHOICE_COMMANDS: &str = "j/k choose · Enter continue · Esc cancel";

fn choice_commands(width: u16) -> &'static str {
    if usize::from(width) >= CHOICE_COMMANDS.chars().count() {
        CHOICE_COMMANDS
    } else {
        "j/k · Enter continue · Esc cancel"
    }
}

pub(super) fn footer_text(app: &App, width: u16) -> String {
    let commands = if usize::from(width) >= COMMANDS.chars().count() {
        COMMANDS
    } else {
        "↵ · n new · a archive · s settings"
    };
    if app.choice_selected.is_some() {
        choice_commands(width).into()
    } else if matches!(app.prompt.as_ref(), Some(Prompt::Archive { .. })) {
        "Enter archive this item · Esc cancel".into()
    } else if app.prompt.is_some() {
        "Enter save · Esc cancel".into()
    } else if !app.message.is_empty() {
        format!("{}\n{commands}", app.message)
    } else {
        commands.into()
    }
}
