//! One module per screen. Each owns its `View` state and provides `draw`, `handle_key`,
//! `handle_mouse` and `crumbs`.

pub(super) mod archive;
pub(super) mod detail;
pub(super) mod list;
pub(super) mod reader;
pub(super) mod scan;
pub(super) mod settings;
