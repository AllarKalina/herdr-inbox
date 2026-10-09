//! The Inbox itself: the selected folders' specs as a tree, with one status column per stage.

use crate::launch;
use crate::store::Result;
use crate::ui::milestone::Milestone;
use crate::ui::{App, ChoicePurpose, Prompt, chrome, modal, text, theme, tree};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};

#[derive(Default)]
pub(crate) struct View {
    pub tree: tree::Tree,
    offset: usize,
    area: Rect,
}

const HINTS: &str = "Enter open · n new · a archive · s settings";

pub(crate) fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Esc => return Ok(true),
        KeyCode::Char('s') => super::settings::open(app),
        KeyCode::Enter if app.list.tree.selected_record().is_some() => super::detail::open(app),
        KeyCode::Enter => app.list.tree.toggle_focused(),
        KeyCode::Char('j') | KeyCode::Down => app.list.tree.move_focus(true),
        KeyCode::Char('k') | KeyCode::Up => app.list.tree.move_focus(false),
        KeyCode::Left => app.list.tree.collapse_or_parent(),
        KeyCode::Right => app.list.tree.expand_or_child(),
        KeyCode::Char('n') => {
            app.choose_client(ChoicePurpose::NewSpec, launch::available_profiles())
        }
        KeyCode::Char('a') => {
            if let Some(id) = app.current().map(|record| record.id.clone()) {
                app.begin(Prompt::Archive { id });
            }
        }
        _ => {}
    }
    Ok(false)
}

pub(crate) fn handle_mouse(app: &mut App, mouse: MouseEvent) -> Result<()> {
    let tree = &mut app.list.tree;
    match mouse.kind {
        MouseEventKind::Moved | MouseEventKind::Down(MouseButton::Left) => {
            let area = app.list.area;
            // The first row of the table is its header.
            let inside = mouse.row > area.y
                && mouse.row < area.bottom()
                && mouse.column >= area.x
                && mouse.column < area.right();
            let index = app.list.offset + usize::from(mouse.row.saturating_sub(area.y + 1));
            if inside && index < tree.rows.len() {
                tree.focused = index;
                if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && tree.rows[index].is_folder
                {
                    tree.toggle_focused();
                }
            }
        }
        MouseEventKind::ScrollDown => tree.move_focus(true),
        MouseEventKind::ScrollUp => tree.move_focus(false),
        _ => {}
    }
    Ok(())
}

pub(crate) fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let panel = modal::panel_height(app);
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(panel),
            Constraint::Length(if app.notice.is_empty() { 1 } else { 2 }),
        ])
        .split(chrome::content(frame.area()));
    draw_table(frame, app, areas[0]);
    if app.records.is_empty() {
        let below_header = Rect::new(
            areas[0].x,
            areas[0].y.saturating_add(2),
            areas[0].width,
            areas[0].height.saturating_sub(2),
        );
        frame.render_widget(
            Paragraph::new("No specs yet.").style(theme::muted()),
            below_header,
        );
    }
    modal::draw_panel(frame, app, areas[1]);
    let footer = chrome::footer_area(frame.area());
    // A failed action must be readable even while its panel is still open.
    if !app.notice.is_empty() {
        let row = Rect::new(footer.x, footer.y.saturating_sub(1), footer.width, 1);
        chrome::draw_notice(frame, &app.notice, row);
    }
    // Narrow popups shorten Enter so every action stays readable.
    let fits = usize::from(footer.width) >= HINTS.chars().count();
    let own = if fits {
        HINTS
    } else {
        "↵ · n new · a archive · s settings"
    };
    chrome::draw_footer(frame, modal::hints(app, footer.width).unwrap_or(own));
}

fn draw_table(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let compact = frame.area().width < 64;
    let jira = app.jira();
    let headers: Vec<&str> = Milestone::visible(jira)
        .iter()
        .map(|stage| stage.column())
        .collect();
    let stages = headers.len() as u16;
    // Compact rows show one icon per stage; full rows give each stage eight cells plus a gap.
    let compact_width = stages * 2 - 1;
    let name_width = area.width.saturating_sub(if compact {
        compact_width + 1
    } else {
        stages * 10
    }) as usize;
    let rows = app
        .list
        .tree
        .rows
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let selected = index == app.list.tree.focused;
            let name = name_cell(app, node, name_width, selected);
            let Some(record) = node.record_index.and_then(|index| app.records.get(index)) else {
                return Row::new(vec![name]);
            };
            let states = Milestone::visible(jira)
                .iter()
                .map(|stage| stage.state(record, jira));
            if compact {
                let mut spans = Vec::new();
                for (index, state) in states.enumerate() {
                    if index > 0 {
                        spans.push(Span::raw(" "));
                    }
                    spans.push(Span::styled(
                        state.list_icon(),
                        if selected {
                            Style::default()
                        } else {
                            Style::default().fg(state.color())
                        },
                    ));
                }
                Row::new(vec![name, Cell::from(Line::from(spans))])
            } else {
                let mut cells = vec![name];
                cells.extend(states.map(|state| {
                    Cell::from(format!("{} {}", state.list_icon(), state.word())).style(
                        if selected {
                            Style::default()
                        } else {
                            Style::default().fg(state.color())
                        },
                    )
                }));
                Row::new(cells)
            }
        })
        .collect::<Vec<_>>();
    let mut widths = vec![Constraint::Fill(1)];
    let mut header = vec![Cell::from("")];
    if compact {
        widths.push(Constraint::Length(compact_width));
        let initials: Vec<_> = headers.iter().map(|name| &name[..1]).collect();
        header.push(Cell::from(initials.join(" ")));
    } else {
        widths.extend(headers.iter().map(|_| Constraint::Length(8)));
        header.extend(headers.iter().map(|name| Cell::from(*name)));
    }
    let header = Row::new(header);
    let table = Table::new(rows, widths)
        .header(header.style(theme::muted().add_modifier(Modifier::BOLD)))
        .column_spacing(if compact { 1 } else { 2 })
        .row_highlight_style(theme::selection());
    let mut state = TableState::default()
        .with_offset(app.list.offset)
        .with_selected((!app.list.tree.rows.is_empty()).then_some(app.list.tree.focused));
    frame.render_stateful_widget(table, area, &mut state);
    app.list.offset = state.offset();
    app.list.area = area;
}

fn name_cell(app: &App, node: &tree::TreeRow, width: usize, selected: bool) -> Cell<'static> {
    let icon = if node.is_folder {
        if node.expanded {
            tree::FOLDER_OPEN
        } else {
            tree::FOLDER_CLOSED
        }
    } else {
        tree::file_icon(&node.path)
    };
    let color = if node.is_folder {
        Color::Yellow
    } else if icon == tree::HTML {
        Color::LightRed
    } else {
        Color::LightBlue
    };
    let prefix = text::fit_prefix(&node.prefix, width.saturating_sub(14));
    let marker = if node.is_folder {
        if node.expanded { "▾ " } else { "▸ " }
    } else {
        "  "
    };
    let unavailable = node
        .record_index
        .and_then(|index| app.records.get(index))
        .is_some_and(|record| !record.spec_path.is_file());
    let title = if unavailable {
        format!("{} [unavailable]", node.label)
    } else {
        node.label.clone()
    };
    let label = text::fit_label(&title, width.saturating_sub(prefix.chars().count() + 4));
    let plain = Style::default();
    let mut spans = vec![
        Span::styled(prefix, if selected { plain } else { theme::faint() }),
        Span::styled(
            format!("{marker}{icon} "),
            if selected { plain } else { plain.fg(color) },
        ),
        Span::styled(
            label,
            if selected {
                plain
            } else if node.is_folder {
                plain.fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                plain
            },
        ),
    ];
    if node.is_folder {
        spans[1].style = spans[1].style.add_modifier(Modifier::BOLD);
    }
    Cell::from(Line::from(spans))
}
