use super::{App, milestone::Milestone, text, theme, tree};
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Row, Table, TableState};

pub(super) fn draw(frame: &mut ratatui::Frame, app: &mut App, area: Rect) {
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
        .tree
        .rows
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let selected = index == app.tree.focused;
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
        .with_offset(app.list_offset)
        .with_selected((!app.tree.rows.is_empty()).then_some(app.tree.focused));
    frame.render_stateful_widget(table, area, &mut state);
    app.list_offset = state.offset();
    app.list_area = area;
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
