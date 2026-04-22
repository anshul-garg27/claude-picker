//! Data layer — session enumeration, pricing, bookmarks, path resolution.
//!
//! The UI agent imports types from here via `crate::data::…` so every
//! user-visible label, number, and list starts life in this module.

pub mod ai_summarize;
pub mod bookmarks;
pub mod budget;
pub mod chains;
pub mod checkpoints;
pub mod claude_json_cache;
pub mod clipboard;
pub mod cost_audit;
pub mod editor;
pub mod file_index;
pub mod mcp_calls;
pub mod path_resolver;
pub mod pinned_projects;
pub mod pricing;
pub mod project;
pub mod replay;
pub mod search_filters;
pub mod session;
pub mod session_rename;
pub mod settings;
pub mod task_queue;
pub mod transcript;

pub use project::Project;
pub use session::{PermissionMode, Session, SessionKind, Usage};
