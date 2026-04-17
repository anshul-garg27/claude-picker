//! Integration tests for `data::file_index`.
//!
//! Writes a miniature `~/.claude/projects/` tree into a temp directory,
//! points `HOME` at it, and runs the loader end-to-end. Exercises the
//! two "moat" queries from the spec:
//!
//! 1. Top-level "which files get touched the most" (the default sort).
//! 2. "Which sessions touched package.json in the last week" (the
//!    `FileIndex::sessions_touching_since` shortcut).
//!
//! We also check the junk filter drops `node_modules` by default.
//!
//! ## Serialisation
//!
//! `FileIndex::build` reads `$HOME`. Cargo runs integration tests in
//! parallel by default — without a shared mutex two tests would race
//! each other's `set_var("HOME", ...)`. Every test that points `$HOME`
//! at a tempdir takes [`HOME_LOCK`] first.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use claude_picker::data::file_index::{is_junk_path, FileIndex};

static HOME_LOCK: Mutex<()> = Mutex::new(());

/// Note: the session id the loader picks up is the JSONL file's *stem*
/// (e.g. `a1.jsonl` → `a1`), not anything we write inside the JSON.
fn mk_session(
    sid: &str,
    file_path: &str,
    timestamp: &str,
    new_string: &str,
    old_string: &str,
) -> String {
    let edit_json = serde_json::json!({
        "type": "tool_use",
        "name": "Edit",
        "id": "t1",
        "input": {
            "file_path": file_path,
            "old_string": old_string,
            "new_string": new_string,
        }
    });
    let assistant_json = serde_json::json!({
        "type": "assistant",
        "timestamp": timestamp,
        "sessionId": sid,
        "message": {
            "role": "assistant",
            "model": "claude-opus-4-7",
            "content": [edit_json],
            "usage": {"input_tokens": 10, "output_tokens": 10}
        }
    });
    let user_line = serde_json::json!({
        "type": "user",
        "entrypoint": "cli",
        "sessionId": sid,
        "message": {"role": "user", "content": "hi"}
    });
    format!(
        "{}\n{}\n",
        serde_json::to_string(&user_line).unwrap(),
        serde_json::to_string(&assistant_json).unwrap()
    )
}

fn setup_home() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::env::set_var("HOME", tmp.path());
    let projects = tmp.path().join(".claude").join("projects");
    fs::create_dir_all(&projects).expect("mkdir projects");

    let proj_a = projects.join("-Users-me-projects-app-a");
    fs::create_dir_all(&proj_a).expect("mkdir proj a");

    // Session a1 touches both package.json and middleware.ts.
    fs::write(
        proj_a.join("a1.jsonl"),
        format!(
            "{}{}",
            mk_session("a1", "/a/package.json", "2026-04-16T10:00:00Z", "a\nb", "a"),
            mk_session(
                "a1",
                "/a/src/auth/middleware.ts",
                "2026-04-16T10:05:00Z",
                "a\nb",
                "a"
            )
        ),
    )
    .unwrap();
    // Session a2 same files, earlier timestamps.
    fs::write(
        proj_a.join("a2.jsonl"),
        format!(
            "{}{}",
            mk_session(
                "a2",
                "/a/package.json",
                "2026-04-15T09:00:00Z",
                "a\nb\nc",
                "a"
            ),
            mk_session(
                "a2",
                "/a/src/auth/middleware.ts",
                "2026-04-15T09:05:00Z",
                "a\nb",
                "a"
            )
        ),
    )
    .unwrap();
    // Session a3 — stale March touch. Outside the "last week" window
    // used by `sessions_touching_package_json_in_last_week`.
    fs::write(
        proj_a.join("a3.jsonl"),
        mk_session("a3", "/a/package.json", "2026-03-01T09:00:00Z", "a", "a"),
    )
    .unwrap();
    // Junk: node_modules — default filter drops this.
    fs::write(
        proj_a.join("a4.jsonl"),
        mk_session(
            "a4",
            "/a/node_modules/pkg/index.js",
            "2026-04-16T11:00:00Z",
            "x",
            "y",
        ),
    )
    .unwrap();

    tmp
}

#[test]
fn build_counts_files_across_sessions() {
    let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _tmp = setup_home();
    let mut idx = FileIndex::build(None, |_| {}).expect("build ok");
    idx.filter_junk();

    // package.json touched by three sessions.
    let pkg = idx
        .files
        .iter()
        .find(|f| f.path == std::path::Path::new("/a/package.json"))
        .expect("package.json present");
    assert_eq!(pkg.session_count, 3);
    assert_eq!(pkg.sessions.len(), 3);

    // middleware.ts touched by two sessions.
    let mw = idx
        .files
        .iter()
        .find(|f| f.path == std::path::Path::new("/a/src/auth/middleware.ts"))
        .expect("middleware.ts present");
    assert_eq!(mw.session_count, 2);

    // node_modules junk was filtered out.
    assert!(!idx
        .files
        .iter()
        .any(|f| f.path == std::path::Path::new("/a/node_modules/pkg/index.js")));
}

#[test]
fn sessions_touching_package_json_in_last_week() {
    let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _tmp = setup_home();
    let mut idx = FileIndex::build(None, |_| {}).expect("build ok");
    idx.filter_junk();

    // Window starts April 9 — excludes the March a3 session.
    let since = chrono::DateTime::parse_from_rfc3339("2026-04-09T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);

    let hits = idx.sessions_touching_since("package.json", since);
    let mut ids: Vec<_> = hits.iter().map(|s| s.session_id.as_str()).collect();
    ids.sort();
    assert_eq!(ids, vec!["a1", "a2"]);
}

#[test]
fn junk_filter_basic() {
    // Pure function, no HOME lock needed.
    assert!(is_junk_path(&PathBuf::from("/a/node_modules/x.js")));
    assert!(is_junk_path(&PathBuf::from("/a/.git/HEAD")));
    assert!(!is_junk_path(&PathBuf::from("/a/src/main.rs")));
}

#[test]
fn project_filter_restricts_scan() {
    let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().expect("tempdir");
    std::env::set_var("HOME", tmp.path());
    let projects = tmp.path().join(".claude").join("projects");
    fs::create_dir_all(&projects).expect("mkdir projects");

    // Two projects. The filter matches on the resolved project name —
    // the basename of the encoded directory's path-resolver output.
    let pa = projects.join("-Users-me-alpha");
    let pb = projects.join("-Users-me-beta");
    fs::create_dir_all(&pa).expect("mkdir alpha");
    fs::create_dir_all(&pb).expect("mkdir beta");
    fs::write(
        pa.join("aaaa.jsonl"),
        mk_session(
            "aaaa",
            "/Users/me/alpha/x.rs",
            "2026-04-16T10:00:00Z",
            "x",
            "y",
        ),
    )
    .unwrap();
    fs::write(
        pb.join("bbbb.jsonl"),
        mk_session(
            "bbbb",
            "/Users/me/beta/y.rs",
            "2026-04-16T10:00:00Z",
            "x",
            "y",
        ),
    )
    .unwrap();

    let idx = FileIndex::build(Some("alpha"), |_| {}).expect("build ok");
    let paths: Vec<_> = idx.files.iter().map(|f| f.path.clone()).collect();
    assert!(paths.contains(&PathBuf::from("/Users/me/alpha/x.rs")));
    assert!(!paths.contains(&PathBuf::from("/Users/me/beta/y.rs")));
}
