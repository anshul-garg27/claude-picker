//! claude-picker library crate — shared types for data, UI, and commands.
//!
//! Module layout:
//!
//! - [`data`] — session enumeration, pricing, bookmarks, path resolution.
//! - [`error`] — typed error enum + crate-wide [`Result`].
//! - [`theme`] — Catppuccin Mocha tokens mapped to ratatui colors.
//! - [`events`] — normalised keyboard/resize events over crossterm.
//! - [`app`] — picker state machine and event loop driver.
//! - [`ui`] — ratatui widgets for every screen (picker, preview, pills, …).
//! - [`commands`] — subcommand dispatchers (default picker, pipe, stats, …).

pub mod app;
pub mod commands;
pub mod data;
pub mod error;
pub mod events;
pub mod theme;
pub mod ui;
