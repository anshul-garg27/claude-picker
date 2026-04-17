//! Data layer — session enumeration, pricing, bookmarks, path resolution.
//!
//! The UI agent imports types from here via `crate::data::…` so every
//! user-visible label, number, and list starts life in this module.

pub mod bookmarks;
pub mod claude_json_cache;
pub mod clipboard;
pub mod editor;
pub mod path_resolver;
pub mod pricing;
pub mod project;
pub mod session;
pub mod session_rename;
pub mod transcript;

pub use project::Project;
pub use session::{PermissionMode, Session, SessionKind, Usage};
