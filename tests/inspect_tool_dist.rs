//! Developer-only inspection test: dumps the tool distribution for the
//! seeded demo session so we can verify the drill-in numbers by hand.
//!
//! Run with `cargo test --release --test inspect_tool_dist -- --nocapture`.
//! Panics at the end so the output is always surfaced.

use std::path::PathBuf;

#[test]
#[ignore = "developer aid: inspects /tmp/claude-picker-demo"]
fn dump_redshift_session_tool_dist() {
    let path = PathBuf::from(
        "/tmp/claude-picker-demo/.claude/projects/\
         -private-tmp-claude-picker-demo-workspace-data-pipeline/2f0e48f8.jsonl",
    );
    let entries = claude_picker::data::tool_dist::collect_tool_distribution(&path);
    eprintln!(
        "\n─── tool distribution for Optimize Redshift COPY command ({} entries) ───",
        entries.len()
    );
    for e in &entries {
        eprintln!(
            "  {:<20} calls={:>3}  out={:>10}  in_after={:>10}",
            e.name, e.usage.call_count, e.usage.output_tokens, e.usage.input_tokens_after
        );
    }
    eprintln!();
}
