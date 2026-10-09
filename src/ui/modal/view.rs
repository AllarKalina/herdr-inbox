//! How an open prompt or client choice is drawn: as a panel in the list, as lines beside a
//! milestone in the detail view, or as a view of its own.

use super::{Choice, ChoicePurpose, Prompt};
use crate::ui::notice::Tone;
use crate::ui::{App, chrome, text, theme};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

const CHOICE_HINTS: &str = "j/k choose · Enter continue · Esc cancel";

fn choice_hints(width: u16) -> &'static str {
    if usize::from(width) >= CHOICE_HINTS.chars().count() {
        CHOICE_HINTS
    } else {
        "j/k · Enter continue · Esc cancel"
    }
}

/// The shortcut line while a modal is open in the list.
pub(crate) fn hints(app: &App, width: u16) -> Option<&'static str> {
    if app.modal.choice.is_some() {
        return Some(choice_hints(width));
    }
    Some(match app.modal.prompt.as_ref()? {
        Prompt::Archive { .. } => "Enter archive this item · Esc cancel",
        _ => "Enter save · Esc cancel",
    })
}

/// Draws the open modal in the panel the list reserves for it.
pub(crate) fn draw_panel(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    if let Some(choice) = &app.modal.choice {
        draw_choices(frame, choice, area);
    } else if let Some(Prompt::Archive { id }) = &app.modal.prompt {
        let detail = match app.record(id) {
            Some(record) => format!(
                "ARCHIVE ITEM\n{}\nID: {}\nSpec: {}\nLinked spec stays in place.\n\
                 Open agent tabs are not closed.",
                record.display_title(),
                record.id,
                record.spec_path.display(),
            ),
            None => "Item disappeared; press Esc to cancel.".into(),
        };
        let block = Block::default()
            .title(" Confirm archive ")
            .borders(Borders::ALL);
        frame.render_widget(Paragraph::new(detail).block(block), area);
    } else if let Some(prompt) = &app.modal.prompt {
        let block = Block::default().title(" Action ").borders(Borders::ALL);
        let line = format!("{}: {}█", prompt.label(), app.modal.input);
        frame.render_widget(Paragraph::new(line).block(block), area);
    }
}

fn draw_choices(frame: &mut ratatui::Frame, choice: &Choice, area: Rect) {
    let title = match choice.purpose {
        ChoicePurpose::Refine { .. } => " Refine spec · Choose client ",
        ChoicePurpose::NewSpec => " Choose client ",
    };
    let items = choice
        .profiles
        .iter()
        .map(|profile| ListItem::new(profile.label()));
    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(theme::selection());
    let mut state = ListState::default().with_selected(Some(choice.selected));
    frame.render_stateful_widget(list, area, &mut state);
}

/// Draws the client choice as a view of its own, in place of the detail view.
pub(crate) fn draw_choice(frame: &mut ratatui::Frame, app: &App) {
    let Some(choice) = &app.modal.choice else {
        return;
    };
    let area = frame.area();
    let width = area.width.saturating_sub(4).min(52);
    let height = area.height.saturating_sub(2).min(9);
    let popup = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    draw_choices(
        frame,
        choice,
        Rect::new(popup.x, popup.y, popup.width, 5.min(popup.height)),
    );
    // A failed launch explains itself directly under the choice it came from.
    if app.notice.tone() == Tone::Error {
        frame.render_widget(
            Paragraph::new(app.notice.text())
                .style(theme::error())
                .wrap(Wrap { trim: false }),
            Rect::new(
                popup.x + 1,
                popup.y + 5,
                popup.width.saturating_sub(2),
                popup.height.saturating_sub(6),
            ),
        );
    }
    let footer = chrome::footer_area(area);
    chrome::draw_footer(frame, choice_hints(footer.width));
}

/// The open prompt as lines for the detail view's milestone controls.
pub(crate) fn rail_lines(app: &App, width: u16) -> Vec<Line<'static>> {
    let Some(prompt) = &app.modal.prompt else {
        return Vec::new();
    };
    let width = usize::from(width);
    let lines = if matches!(prompt, Prompt::Settle { .. }) {
        vec![
            "Session ended?".into(),
            "No tabs closed.".into(),
            "Enter settle".into(),
            "Esc cancel".into(),
        ]
    } else {
        // Long input scrolls so the cursor stays visible.
        let count = app.modal.input.chars().count();
        let tail: String = app
            .modal
            .input
            .chars()
            .skip(count.saturating_sub(width.saturating_sub(1)))
            .collect();
        vec![
            text::fit_label(prompt.label(), width),
            format!("{tail}█"),
            "Enter save · Esc cancel".into(),
        ]
    };
    lines.into_iter().map(Line::from).collect()
}
