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

// These clippy lints are expected trade-offs for a feature-rich TUI crate:
// - `large_enum_variant`: TreeNode deliberately stores owned Session/Project
//   variants; boxing every variant would hurt ergonomics with no perf gain
//   since these live inside Vecs we already heap-allocate.
#![allow(clippy::large_enum_variant)]

pub mod app;
pub mod commands;
pub mod config;
pub mod data;
pub mod error;
pub mod events;
pub mod resume;
pub mod theme;
pub mod ui;
