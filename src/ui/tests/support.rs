//! Off-screen rendering shared by the UI tests.

use super::*;
use ratatui::buffer::Cell;

/// Renders the current screen and returns its cells row by row, with their styling.
pub(super) fn cells(app: &mut App, width: u16, height: u16) -> Result<Vec<Vec<Cell>>> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| draw::draw(frame, app))?;
    Ok(terminal
        .backend()
        .buffer()
        .content()
        .chunks(usize::from(width).max(1))
        .map(<[Cell]>::to_vec)
        .collect())
}

pub(super) fn text(cells: &[Vec<Cell>]) -> Vec<String> {
    cells
        .iter()
        .map(|row| row.iter().map(Cell::symbol).collect())
        .collect()
}

/// Renders the current screen and returns its text, one string per row.
pub(super) fn lines(app: &mut App, width: u16, height: u16) -> Result<Vec<String>> {
    Ok(text(&cells(app, width, height)?))
}
