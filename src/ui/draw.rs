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
    if app.screen == Screen::Trash {
        super::trash::draw(frame, app);
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
            frame.render_widget(
                Paragraph::new("j/k choose").style(Style::default().fg(Color::Gray)),
                Rect::new(
                    popup.x + 1,
                    popup.bottom().saturating_sub(1),
                    popup.width.saturating_sub(2),
                    1,
                ),
            );
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
    let footer_area = Rect::new(
        areas[2].x.saturating_sub(1),
        areas[2].y,
        areas[2].width.saturating_add(2),
        areas[2].height,
    );
    frame.render_widget(
        Paragraph::new(if footer_area.width < COMMANDS.chars().count() as u16 {
            footer_text(app).replace("Enter open", "Enter")
        } else {
            footer_text(app)
        }),
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

pub(super) fn footer_text(app: &App) -> String {
    if app.choice_selected.is_some() {
        "j/k choose · Enter continue · Esc cancel".into()
    } else if matches!(app.prompt.as_ref(), Some(Prompt::Archive { .. })) {
        "Enter archive this item · Esc cancel".into()
    } else if app.prompt.is_some() {
        "Enter save · Esc cancel".into()
    } else if !app.message.is_empty() {
        format!("{}\n{COMMANDS}", app.message)
    } else {
        COMMANDS.into()
    }
}
