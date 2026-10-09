//! UI tests. Rendering is pinned by the golden snapshots; the other files test behaviour:
//! what keys and clicks do to the state and to the store.

use super::*;
use crate::launch::Profile;
use crate::store::Change;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

mod feedback;
mod flows;
mod golden;
mod mouse;
mod refinement;
mod resize;
mod scope;
mod settings;
mod support;
mod timeline;
mod tree;

/// Sends a key the way the event loop does: a failed action becomes the notice.
fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    Ok(dispatch(app, Event::Key(key)))
}

fn handle_mouse(app: &mut App, mouse: MouseEvent) -> Result<()> {
    dispatch(app, Event::Mouse(mouse));
    Ok(())
}
