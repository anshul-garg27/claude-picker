//! Subcommand dispatchers.
//!
//! Each file here is a thin glue layer: parse context, call into the data
//! layer, hand the result to the UI layer. The goal is that `main.rs` only
//! ever has to wire CLI → command-fn; the command implementations carry
//! their own dependencies.

pub mod ai_titles_cmd;
pub mod audit_cmd;
pub mod checkpoints_cmd;
pub mod diff_cmd;
pub mod files_cmd;
pub mod hooks_cmd;
pub mod mcp_cmd;
pub mod pick;
pub mod pipe_cmd;
pub mod search_cmd;
pub mod stats_cmd;
pub mod tree_cmd;
