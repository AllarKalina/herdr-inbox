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
        // Clients, a blank row, the topic: the same rows in both steps, so nothing moves.
        (Some(choice), _) => choice.profiles.len() as u16 + 4,
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

/// A row's label. A step that has not been reached yet recedes with its row, so it does not
/// compete with the one being answered; a step already answered keeps its label.
fn row_label(text: &'static str, live: bool) -> Span<'static> {
    let style = if live { theme::muted() } else { theme::faint() };
    Span::styled(format!("{text:<LABEL_WIDTH$}"), style)
}

/// One client per row, the chosen one marked the way the detail view marks its chosen
/// action. Once the choice is `settled` the rows stay exactly where they were and go quiet.
fn client_lines(choice: &Choice, width: u16, settled: bool) -> Vec<Line<'static>> {
    let room = usize::from(width).saturating_sub(LABEL_WIDTH + 2);
    choice
        .profiles
        .iter()
        .enumerate()
        .map(|(index, profile)| {
            let label = if index == 0 { "Client" } else { "" };
            let name = fit_client(profile.label(), room);
            let chosen = index == choice.selected;
            let option = match (chosen, settled) {
                (true, false) => Span::styled(format!("✦ {name}"), theme::chosen_action()),
                (false, false) => Span::styled(format!("  {name}"), theme::action()),
                (true, true) => Span::raw(format!("✦ {name}")),
                (false, true) => Span::styled(format!("  {name}"), theme::faint()),
            };
            // The label keeps its strength once the choice is made: it reads as done, not gone.
            Line::from(vec![row_label(label, true), option])
        })
        .collect()
}

/// A new spec in two steps: choose the client, then say what the interview is about. Both
/// steps draw the same rows in the same places; only which part is live changes.
fn draw_new_spec(frame: &mut ratatui::Frame, choice: &Choice, topic: &str, area: Rect) {
    let typing = choice.entering_topic;
    let block = chrome::panel(if typing {
        " New spec · Topic "
    } else {
        " New spec · Client "
    });
    let inner = block.inner(area);
    let mut lines = client_lines(choice, inner.width, typing);
    lines.push(Line::default());
    let mut topic_line = vec![row_label("Topic", typing)];
    // The topic scrolls so the cursor stays visible however much is typed.
    let room = usize::from(inner.width).saturating_sub(LABEL_WIDTH + 1);
    let count = topic.chars().count();
    let shown: String = topic.chars().skip(count.saturating_sub(room)).collect();
    match (typing, topic.is_empty()) {
        (true, true) => {
            topic_line.push(Span::raw("█"));
            topic_line.push(Span::styled(" optional", theme::muted()));
        }
        (true, false) => topic_line.push(Span::raw(format!("{shown}█"))),
        // Not this step's business yet: shown faintly so the next step is no surprise.
        (false, true) => topic_line.push(Span::styled("optional", theme::faint())),
        (false, false) => topic_line.push(Span::styled(shown, theme::faint())),
    }
    lines.push(Line::from(topic_line));
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
    let lines = client_lines(choice, block.inner(area).width, false);
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
