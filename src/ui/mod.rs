//! UI layer — ratatui widgets and screens.
//!
//! The top-level entry point is [`picker::render`], which dispatches on the
//! current [`crate::app::Mode`]. Per-pane modules are kept small and single-
//! purpose so swapping one out (e.g. reimplementing the preview to show the
//! full transcript in a scroll pane) doesn't ripple through the others.

pub mod actions;
pub mod command_palette;
pub mod conversation_viewer;
pub mod diff;
pub mod footer;
pub mod help_overlay;
pub mod layout;
pub mod model_pill;
pub mod picker;
pub mod preview;
pub mod project_list;
pub mod rename_modal;
pub mod search;
pub mod session_list;
pub mod stats;
pub mod text;
pub mod tree;
