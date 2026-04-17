//! Integration tests for the transcript parser used by the conversation viewer.
//!
//! Round-trips the shared `tests/fixtures/sample.jsonl` through
//! [`claude_picker::data::transcript::load_transcript`] to ensure the parser
//! handles the same shapes the aggregator understands, then drills into a
//! synthetic fixture with tool_use / tool_result / thinking blocks to prove
//! every content variant round-trips.

use std::path::PathBuf;

use claude_picker::data::transcript::{load_transcript, ContentItem, Role};

fn sample_fixture() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/sample.jsonl");
    p
}

#[test]
fn loads_real_jsonl_and_preserves_order() {
    let messages = load_transcript(&sample_fixture()).expect("ok");
    // 2 user + 2 assistant = 4 messages in the fixture.
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[0].role, Role::User);
    assert_eq!(messages[1].role, Role::Assistant);
    assert_eq!(messages[2].role, Role::User);
    assert_eq!(messages[3].role, Role::Assistant);
}

#[test]
fn mixed_content_blocks_parse_into_typed_items() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("t.jsonl");
    std::fs::write(
        &path,
        concat!(
            r#"{"type":"user","message":{"role":"user","content":"explain"}}"#, "\n",
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"hmm"},{"type":"text","text":"ok"},{"type":"tool_use","name":"Edit","id":"tu1","input":{"file_path":"/a/b.rs"}}]}}"#, "\n",
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu1","content":"12 lines"},{"type":"text","text":"thanks"}]}}"#, "\n",
        ),
    )
    .expect("write");
    let messages = load_transcript(&path).expect("ok");
    assert_eq!(messages.len(), 3);

    // Message 1: three blocks — thinking, text, tool_use.
    assert_eq!(messages[1].items.len(), 3);
    assert!(matches!(messages[1].items[0], ContentItem::Thinking { .. }));
    assert!(matches!(messages[1].items[1], ContentItem::Text(_)));
    match &messages[1].items[2] {
        ContentItem::ToolUse { name, .. } => assert_eq!(name, "Edit"),
        other => panic!("wrong variant: {other:?}"),
    }

    // Message 2: tool_result + text (user replying with output).
    assert_eq!(messages[2].items.len(), 2);
    assert!(matches!(
        messages[2].items[0],
        ContentItem::ToolResult { .. }
    ));
}

#[test]
fn empty_or_non_message_lines_are_skipped() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("t.jsonl");
    std::fs::write(
        &path,
        concat!(
            r#"{"type":"custom-title","customTitle":"x"}"#,
            "\n",
            "\n",
            r#"{"type":"permission-mode","permissionMode":"plan"}"#,
            "\n",
            r#"{"type":"user","message":{"role":"user","content":"real"}}"#,
            "\n",
        ),
    )
    .expect("write");
    let messages = load_transcript(&path).expect("ok");
    assert_eq!(messages.len(), 1);
}

#[test]
fn as_plain_text_contains_tool_use_summary() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("t.jsonl");
    std::fs::write(
        &path,
        concat!(
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"hi"},{"type":"tool_use","name":"Read","id":"x","input":{"file_path":"/foo.rs"}}]}}"#,
            "\n",
        ),
    ).expect("write");
    let messages = load_transcript(&path).expect("ok");
    let plain = messages[0].as_plain_text();
    assert!(plain.contains("hi"));
    assert!(plain.contains("tool_use: Read"));
    assert!(plain.contains("/foo.rs"));
}
