//! Integration tests for the `--tree` screen's data flatten step.
//!
//! The renderer itself is exercised inside `src/ui/tree.rs`'s unit tests;
//! this file verifies the public flatten API on realistic `Session`
//! inputs — a three-session project with one fork and one unrelated root
//! — and asserts the exact order, depths, and connector strings.

use std::path::PathBuf;

use claude_picker::data::pricing::TokenCounts;
use claude_picker::data::session::SessionKind;
use claude_picker::data::{Project, Session};
use claude_picker::ui::tree::{build_tree, connector_prefix, NodeKind};

fn mk_session(
    id: &str,
    forked_from: Option<&str>,
    name: Option<&str>,
    ts_offset_secs: i64,
) -> Session {
    let base = chrono::Utc::now();
    Session {
        id: id.to_string(),
        project_dir: PathBuf::from("/tmp/proj"),
        name: name.map(|s| s.to_string()),
        auto_name: None,
        message_count: 5,
        tokens: TokenCounts::default(),
        total_cost_usd: 0.0,
        model_summary: "claude-opus-4-7".to_string(),
        first_timestamp: Some(base),
        last_timestamp: Some(base + chrono::Duration::seconds(ts_offset_secs)),
        is_fork: forked_from.is_some(),
        forked_from: forked_from.map(|s| s.to_string()),
        entrypoint: SessionKind::Cli,
    }
}

fn mk_project() -> Project {
    Project {
        name: "proj".to_string(),
        path: PathBuf::from("/tmp/proj"),
        encoded_dir: "-tmp-proj".to_string(),
        session_count: 0,
        last_activity: None,
        git_branch: None,
    }
}

#[test]
fn three_session_tree_flattens_correctly() {
    // Build: one root (`auth-refactor`), one fork of that root, one
    // unrelated root. Timestamps are chosen so the recency sort places
    // the forked-root first and unrelated second.
    let sessions = vec![
        mk_session("root1", None, Some("auth-refactor"), 100),
        mk_session("fork1", Some("root1"), Some("auth-refactor-retry"), 50),
        mk_session("other", None, Some("drizzle-migration"), 80),
    ];
    let projects = vec![mk_project()];
    let nodes = build_tree(&projects, &[sessions]);

    // Expected layout:
    //   0: project header
    //   1: root1             (depth 1, ├─)
    //   2: fork1             (depth 2, └─)
    //   3: other             (depth 1, └─)
    assert_eq!(nodes.len(), 4);
    assert!(matches!(nodes[0].kind, NodeKind::ProjectHeader { .. }));

    // Row 1: root1 at depth 1. Not the last root (`other` still to come)
    // so its connector should be a branch.
    match &nodes[1].kind {
        NodeKind::SessionRow { session } => assert_eq!(session.id, "root1"),
        _ => panic!("expected session row at index 1"),
    }
    assert_eq!(nodes[1].depth, 1);
    assert!(!nodes[1].is_last_child, "root1 has a sibling after it");
    assert_eq!(connector_prefix(&nodes[1]), "├─ ");

    // Row 2: fork1 at depth 2. Only child of root1 → is_last_child=true.
    // Its ancestor_bars is `[false]` because its parent (root1) is NOT
    // the last root, so the vertical bar continues.
    match &nodes[2].kind {
        NodeKind::SessionRow { session } => assert_eq!(session.id, "fork1"),
        _ => panic!("expected session row at index 2"),
    }
    assert_eq!(nodes[2].depth, 2);
    assert!(nodes[2].is_last_child, "fork1 is the only child of root1");
    assert_eq!(nodes[2].ancestor_bars, vec![false]);
    assert_eq!(connector_prefix(&nodes[2]), "│  └─ ");

    // Row 3: other at depth 1, last root.
    match &nodes[3].kind {
        NodeKind::SessionRow { session } => assert_eq!(session.id, "other"),
        _ => panic!("expected session row at index 3"),
    }
    assert_eq!(nodes[3].depth, 1);
    assert!(nodes[3].is_last_child, "other is the last root");
    assert_eq!(connector_prefix(&nodes[3]), "└─ ");
}
