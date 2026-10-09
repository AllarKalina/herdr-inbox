//! One spec: its preview beside the progress rail, where each milestone offers its actions.

mod rail;

use super::reader;
use crate::launch;
use crate::store::{Change, Record, Result};
use crate::ui::{
    App, ChoicePurpose, DetailAction, Milestone, MilestoneFeedback, Prompt, Screen, chrome, modal,
};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Paragraph, Wrap};
use std::process::Command;

pub(crate) struct View {
    /// The open item's ID. An ID survives refreshes that reorder or replace the records.
    pub record: Option<String>,
    pub milestone: Milestone,
    /// Index of the action Enter would run among the selected milestone's actions.
    pub action: usize,
    pub milestone_hitboxes: Vec<(Milestone, Rect)>,
    pub action_hitboxes: Vec<Rect>,
    pub feedback: Option<MilestoneFeedback>,
}

impl Default for View {
    fn default() -> Self {
        Self {
            record: None,
            milestone: Milestone::Spec,
            action: 0,
            milestone_hitboxes: Vec::new(),
            action_hitboxes: Vec::new(),
            feedback: None,
        }
    }
}

const HINTS: &str = "j/k stage · Tab action · r read";

/// Opens the item selected in the list, on its next actionable milestone.
pub(crate) fn open(app: &mut App) {
    let Some(id) = app.current().map(|record| record.id.clone()) else {
        return;
    };
    app.detail.record = Some(id);
    app.screen = Screen::Detail;
    app.notice.clear();
    if let Some(next) = app.next_milestone() {
        app.select_milestone(next);
    }
}

/// Keeps the view consistent after records were reread: leaves when the item is gone,
/// follows the recommended milestone when it advances, and never rests on a hidden stage.
pub(crate) fn reconcile(app: &mut App, was_next: Option<Milestone>) {
    let Some(index) = app
        .detail
        .record
        .as_deref()
        .and_then(|id| app.records.iter().position(|record| record.id == id))
    else {
        app.screen = Screen::List;
        app.detail.feedback = None;
        app.notice.info("Item no longer in inbox");
        return;
    };
    // Going back to the list lands on the item that was open.
    app.list.tree.focus_record(index);
    let next = app.next_milestone();
    let advanced = was_next != next && was_next == Some(app.detail.milestone);
    let hidden = !Milestone::visible(app.jira()).contains(&app.detail.milestone);
    if let Some(next) = next
        && (advanced || hidden)
    {
        app.focus_milestone(next);
    }
    app.detail.action = app.detail.action.min(app.actions().len().saturating_sub(1));
}

/// The spec's domain folders, then its title.
pub(crate) fn crumbs(app: &App) -> Vec<String> {
    let Some(record) = app.current() else {
        return Vec::new();
    };
    let rows = &app.list.tree.rows;
    let position = rows.iter().position(|row| {
        row.record_index
            .is_some_and(|index| app.records[index].id == record.id)
    });
    let mut crumbs = Vec::new();
    if let Some(position) = position {
        let mut depth = rows[position].depth;
        for row in rows[..position].iter().rev() {
            if row.depth < depth && row.is_folder {
                crumbs.push(row.label.clone());
                depth = row.depth;
            }
        }
        crumbs.reverse();
    }
    crumbs.push(record.display_title().into());
    crumbs
}

impl App {
    pub(crate) fn actions(&self) -> Vec<DetailAction> {
        self.current().map_or_else(Vec::new, |record| {
            self.detail.milestone.actions(record, self.jira())
        })
    }

    /// Selects a milestone on the user's behalf, which dismisses any acknowledgement.
    pub(crate) fn select_milestone(&mut self, milestone: Milestone) {
        self.detail.feedback = None;
        self.focus_milestone(milestone);
    }

    fn focus_milestone(&mut self, milestone: Milestone) {
        self.detail.milestone = milestone;
        self.detail.action = 0;
        self.detail.action_hitboxes.clear();
    }

    fn move_milestone(&mut self, forward: bool) {
        let stages = Milestone::visible(self.jira());
        let index = stages
            .iter()
            .position(|stage| *stage == self.detail.milestone)
            .unwrap_or(0);
        let next = if forward {
            (index + 1).min(stages.len() - 1)
        } else {
            index.saturating_sub(1)
        };
        self.select_milestone(stages[next]);
    }

    /// Shows a completed step beside its milestone instead of in the notice row.
    pub(crate) fn acknowledge(&mut self, milestone: Milestone) {
        self.detail.feedback = Some(MilestoneFeedback { milestone });
        self.notice.clear();
    }
}

pub(crate) fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    let actions = app.actions();
    match key.code {
        KeyCode::Esc => {
            app.detail.feedback = None;
            app.screen = Screen::List;
        }
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Char('j') | KeyCode::Down => app.move_milestone(true),
        KeyCode::Char('k') | KeyCode::Up => app.move_milestone(false),
        KeyCode::Char('r') => reader::open(app),
        KeyCode::Char('x') => {
            let active = app.current().filter(|record| record.active_spec_session());
            if let Some(id) = active.map(|record| record.id.clone()) {
                app.begin(Prompt::Settle { id });
            }
        }
        KeyCode::Char('o') if app.jira() => run(app, DetailAction::OpenJira)?,
        KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') if !actions.is_empty() => {
            app.detail.action = (app.detail.action + 1) % actions.len();
        }
        KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') if !actions.is_empty() => {
            app.detail.action = (app.detail.action + actions.len() - 1) % actions.len();
        }
        KeyCode::Enter if !actions.is_empty() => run(app, actions[app.detail.action])?,
        _ => {}
    }
    Ok(false)
}

pub(crate) fn handle_mouse(app: &mut App, mouse: MouseEvent) -> Result<()> {
    let clicked = mouse.kind == MouseEventKind::Down(MouseButton::Left);
    match mouse.kind {
        MouseEventKind::ScrollDown => app.move_milestone(true),
        MouseEventKind::ScrollUp => app.move_milestone(false),
        _ => {}
    }
    // A milestone answers to its label row and to the five-by-three node area around it.
    let stage = app.detail.milestone_hitboxes.iter().find(|(_, area)| {
        (mouse.row == area.y && mouse.column >= area.x && mouse.column < area.right())
            || (mouse.column >= area.x + 5
                && mouse.column <= area.x + 9
                && mouse.row >= area.y.saturating_sub(1)
                && mouse.row <= area.y + 1)
    });
    if clicked && let Some((stage, _)) = stage {
        app.select_milestone(*stage);
        return Ok(());
    }
    let hovered = app
        .detail
        .action_hitboxes
        .iter()
        .position(|area| area.contains((mouse.column, mouse.row).into()));
    if let Some(index) = hovered {
        app.detail.action = index;
        if clicked && let Some(action) = app.actions().get(index).copied() {
            run(app, action)?;
            app.refresh()?;
        }
    }
    Ok(())
}

/// Runs a milestone action: most open a prompt, some act at once.
fn run(app: &mut App, action: DetailAction) -> Result<()> {
    let Some(record) = app.current().cloned() else {
        return Ok(());
    };
    let id = record.id.clone();
    match action {
        DetailAction::Finish if record.title.is_empty() => app.begin(Prompt::FinishTitle { id }),
        DetailAction::Finish => {
            let updated = app.store.update(&id, Change::Finish { title: None })?;
            let _ = launch::rename_tab(&updated);
            app.acknowledge(Milestone::Spec);
        }
        DetailAction::ReadSpec => reader::open(app),
        DetailAction::RefineSpec => {
            app.choose_client(ChoicePurpose::Refine { id }, launch::available_profiles())
        }
        DetailAction::Jira => app.begin(Prompt::Jira { id }),
        DetailAction::UpdateJira => {
            app.begin_with(Prompt::Jira { id }, record.jira.key.unwrap_or_default())
        }
        DetailAction::OpenJira => {
            if let Some(url) = record.jira.url.as_deref() {
                open_url(url, "Jira ticket")?;
                app.notice.success("Opened Jira ticket");
            }
        }
        DetailAction::Implement => app.begin(Prompt::Agent { id }),
        DetailAction::UpdateImplementation => app.begin_with(
            Prompt::Agent { id },
            record.implementation.agent.unwrap_or_default(),
        ),
        DetailAction::Pr => app.begin(Prompt::Pr { id }),
        DetailAction::UpdatePr => {
            app.begin_with(Prompt::Pr { id }, record.pr.url.unwrap_or_default())
        }
        DetailAction::ReviewPr => {
            if let Some(url) = record.pr.url.as_deref() {
                open_url(url, "draft PR")?;
                app.notice.success("Opened draft PR");
            }
        }
    }
    Ok(())
}

fn open_url(url: &str, what: &str) -> Result<()> {
    if !Command::new("open").arg(url).status()?.success() {
        return Err(format!("Could not open {what}").into());
    }
    Ok(())
}

pub(crate) fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    if app.modal.choice.is_some() {
        return modal::draw_choice(frame, app);
    }
    let Some(record) = app.current().cloned() else {
        return;
    };
    let mut body = chrome::content(frame.area());
    // Short popups give the bottom inset to the progress controls.
    let short = body.height < 20;
    if short {
        body.height = body.height.saturating_add(1);
    }
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(if short { 2 } else { 3 }),
        ])
        .split(body);
    if body.width >= 78 {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(40),
                Constraint::Length(2),
                Constraint::Length(36),
            ])
            .split(areas[0]);
        draw_preview(frame, &record, columns[0]);
        rail::draw(frame, app, &record, columns[2]);
    } else {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(if areas[0].height < 18 { 0 } else { 3 }),
                Constraint::Length(15.min(areas[0].height)),
            ])
            .split(areas[0]);
        draw_preview(frame, &record, rows[0]);
        rail::draw(frame, app, &record, rows[1]);
    }
    chrome::draw_footer(frame, HINTS);
    if !app.notice.is_empty() {
        // Roomy layouts reserve the row above the shortcuts; short ones use the reclaimed inset.
        let hints = chrome::footer_area(frame.area());
        let y = if short {
            hints.y.saturating_add(1)
        } else {
            hints.y.saturating_sub(1)
        };
        chrome::draw_notice(frame, &app.notice, Rect::new(hints.x, y, hints.width, 1));
    }
}

fn draw_preview(frame: &mut ratatui::Frame, record: &Record, area: Rect) {
    let text = reader::spec_text(record);
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
