//! UI layer — ratatui widgets and screens.
//!
//! The top-level entry point is [`picker::render`], which dispatches on the
//! current [`crate::app::Mode`]. Per-pane modules are kept small and single-
//! purpose so swapping one out (e.g. reimplementing the preview to show the
//! full transcript in a scroll pane) doesn't ripple through the others.

pub mod actions;
pub mod audit;
// Drill-in detail overlay for the cost audit (#16).
pub mod audit_detail;
// Always-visible row-0 header shared by picker + viewer screens.
pub mod breadcrumb;
pub mod checkpoints;
pub mod command_palette;
pub mod conversation_viewer;
pub mod diff;
pub mod files;
pub mod filter_ribbon;
pub mod footer;
// Shared tachyonfx effect helpers (reduce-motion-aware). Every UI feature
// that opts into an animation pulls helpers from here so the reduce-motion
// path is enforced in one place.
pub mod fx;
pub mod heatmap;
pub mod help_overlay;
pub mod hooks;
pub mod layout;
// ASCII masthead rendered on `--help` and first-run onboarding (#48, POOL-6).
pub mod masthead;
pub mod mcp;
pub mod model_pill;
pub mod model_simulator;
pub mod onboarding;
pub mod picker;
pub mod preview;
pub mod project_list;
pub mod rename_modal;
pub mod replay;
pub mod search;
pub mod session_list;
pub mod stats;
pub mod subagent_tree;
pub mod task_drawer;
pub mod text;
// Shared timestamp formatter used by preview + conversation viewer (#9, FEAT-7).
pub mod timestamp_fmt;
// F2/E17 project thumbnails: identicon renderer + in-memory LRU cache. The
// cache lives next to the renderer so `project_list` only needs one import
// path. The renderer emits Unicode halfblocks on every terminal — no
// graphics-protocol probe, no C library dependencies.
pub mod thumbnail;
pub mod thumbnail_cache;
pub mod tree;
pub mod which_key;
