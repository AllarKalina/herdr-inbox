//! How an open prompt or client choice is drawn: as a panel in the list, as lines beside a
//! milestone in the detail view, or as a view of its own.

use super::{Choice, ChoicePurpose, Prompt};
use crate::ui::notice::Tone;
use crate::ui::{App, chrome, text, theme};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

/// The shortcut line for the open modal, shortened when the popup is narrow.
pub(crate) fn hints(app: &App, width: u16) -> Option<&'static str> {
    let fits = |text: &str| usize::from(width) >= text.chars().count();
    let pick = |full: &'static str, short: &'static str| if fits(full) { full } else { short };
    if let Some(choice) = &app.modal.choice {
        return Some(match choice.purpose {
            ChoicePurpose::NewSpec if choice.entering_topic => "Enter start session · Esc back",
            ChoicePurpose::NewSpec => pick(
                "j/k choose · Enter next · Esc cancel",
                "j/k · Enter next · Esc cancel",
            ),
            ChoicePurpose::Refine { .. } => pick(
                "j/k choose · Enter start session · Esc cancel",
                "j/k · Enter start · Esc cancel",
            ),
        });
    }
    Some(match app.modal.prompt.as_ref()? {
        Prompt::Archive { .. } => "Enter archive this item · Esc cancel",
        _ => "Enter save · Esc cancel",
    })
}

/// Rows the list gives the open modal: its content and its border, nothing spare.
pub(crate) fn panel_height(app: &App) -> u16 {
    match (&app.modal.choice, &app.modal.prompt) {
        // The chosen client, a blank row, the topic.
        (Some(choice), _) if choice.entering_topic => 5,
        (Some(choice), _) => choice.profiles.len() as u16 + 2,
        (None, Some(_)) => 8,
        (None, None) => 0,
    }
}

/// Draws the open modal in the panel the list reserves for it.
pub(crate) fn draw_panel(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    if let Some(choice) = &app.modal.choice {
        draw_new_spec(frame, choice, &app.modal.input, area);
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
        let block = chrome::panel(" Confirm archive ");
        frame.render_widget(Paragraph::new(detail).block(block), area);
    }
}

const LABEL_WIDTH: usize = 8;

/// A client's label in `room` cells: trailing details go first, then the text is cut.
fn fit_client(label: &str, room: usize) -> String {
    let mut label = label;
    while label.chars().count() > room {
        match label.rsplit_once(" · ") {
            Some((shorter, _)) => label = shorter,
            None => break,
        }
    }
    text::fit_label(label, room)
}

/// One client per row, the chosen one marked the way the detail view marks its chosen action.
fn client_lines(choice: &Choice, width: u16) -> Vec<Line<'static>> {
    let room = usize::from(width).saturating_sub(LABEL_WIDTH + 2);
    choice
        .profiles
        .iter()
        .enumerate()
        .map(|(index, profile)| {
            let label = if index == 0 { "Client" } else { "" };
            let name = fit_client(profile.label(), room);
            let option = if index == choice.selected {
                Span::styled(format!("✦ {name}"), theme::chosen_action())
            } else {
                Span::styled(format!("  {name}"), theme::action())
            };
            Line::from(vec![
                Span::styled(format!("{label:<LABEL_WIDTH$}"), theme::muted()),
                option,
            ])
        })
        .collect()
}

/// A new spec in two steps: choose the client, then say what the interview is about.
fn draw_new_spec(frame: &mut ratatui::Frame, choice: &Choice, topic: &str, area: Rect) {
    if !choice.entering_topic {
        let block = chrome::panel(" New spec · Client ");
        let lines = client_lines(choice, block.inner(area).width);
        return frame.render_widget(Paragraph::new(lines).block(block), area);
    }
    let block = chrome::panel(" New spec · Topic ");
    let inner = block.inner(area);
    let value = usize::from(inner.width).saturating_sub(LABEL_WIDTH);
    let label = |text: &'static str| Span::styled(format!("{text:<LABEL_WIDTH$}"), theme::muted());
    // The client is settled; it stays in view so the choice is not taken on trust.
    let client = fit_client(choice.profiles[choice.selected].label(), value);
    // The topic scrolls so the cursor stays visible however much is typed.
    let count = topic.chars().count();
    let shown: String = topic
        .chars()
        .skip(count.saturating_sub(value.saturating_sub(1)))
        .collect();
    let mut topic_line = vec![label("Topic"), Span::raw(format!("{shown}█"))];
    if topic.is_empty() {
        topic_line.push(Span::styled(" optional", theme::muted()));
    }
    let lines = vec![
        Line::from(vec![label("Client"), Span::raw(client)]),
        Line::default(),
        Line::from(topic_line),
    ];
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// Draws the client choice as a view of its own, in place of the detail view.
pub(crate) fn draw_choice(frame: &mut ratatui::Frame, app: &App) {
    let Some(choice) = &app.modal.choice else {
        return;
    };
    let area = frame.area();
    let width = area.width.saturating_sub(4).min(52);
    let rows = choice.profiles.len() as u16 + 2;
    let height = area.height.saturating_sub(2).min(rows + 4);
    let popup = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    let block = chrome::panel(" Refine spec ");
    let area = Rect::new(popup.x, popup.y, popup.width, rows.min(popup.height));
    let lines = client_lines(choice, block.inner(area).width);
    frame.render_widget(Paragraph::new(lines).block(block), area);
    // A failed launch explains itself directly under the choice it came from.
    if app.notice.tone() == Tone::Error {
        frame.render_widget(
            Paragraph::new(app.notice.text())
                .style(theme::error())
                .wrap(Wrap { trim: false }),
            Rect::new(
                popup.x + 1,
                popup.y + rows,
                popup.width.saturating_sub(2),
                popup.height.saturating_sub(rows),
            ),
        );
    }
    let footer = chrome::footer_area(area);
    if let Some(hints) = hints(app, footer.width) {
        chrome::draw_footer(frame, hints);
    }
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
