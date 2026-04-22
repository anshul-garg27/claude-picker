//! Data layer — session enumeration, pricing, bookmarks, path resolution.
//!
//! The UI agent imports types from here via `crate::data::…` so every
//! user-visible label, number, and list starts life in this module.

pub mod ai_summarize;
// Deterministic cost-anomaly detector for the session list (#29 + POOL-4).
pub mod anomaly;
pub mod bookmarks;
pub mod budget;
pub mod chains;
pub mod checkpoints;
pub mod claude_json_cache;
pub mod clipboard;
pub mod cost_audit;
pub mod editor;
pub mod file_index;
pub mod marks;
pub mod mcp_calls;
pub mod path_resolver;
pub mod pinned_projects;
pub mod pricing;
pub mod project;
// Secret/PII redaction for preview pane + export (#53, POOL-1).
pub mod redact;
pub mod replay;
pub mod search_filters;
pub mod session;
pub mod session_rename;
pub mod settings;
pub mod task_queue;
// Per-tool distribution for the cost-audit drill-in detail view (#16).
pub mod tool_dist;
pub mod transcript;

pub use project::Project;
pub use session::{PermissionMode, Session, SessionKind, Usage};
