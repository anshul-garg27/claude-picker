//! End-to-end test for the `--stats` aggregation pipeline.
//!
//! Builds a synthetic `~/.claude/projects/` layout in a tempdir with three
//! sessions across two projects, runs `aggregate_from_dirs`, and asserts
//! that every field of the resulting [`StatsData`] matches hand-computed
//! expectations. This covers the data path that the dashboard renders from
//! without touching the Ratatui layer — render tests would need a virtual
//! backend which costs more lines than the coverage buys us at v2.

use std::fs;
use std::path::Path;

use chrono::NaiveDate;

use claude_picker::commands::stats_cmd::aggregate_from_dirs;
use claude_picker::data::pricing::Family;

/// One session JSONL with two assistant messages on Opus 4.7.
///
/// Per Opus 4.7 pricing ($5 in / $25 out / $6.25 cw5 / $0.50 cr):
///   msg A: 1000 in + 500 out + 2000 cw5
///     = 0.005 + 0.0125 + 0.0125 = 0.030 USD
///   msg B: 500 in + 1500 out + 500 cr
///     = 0.0025 + 0.0375 + 0.00025 = 0.04025 USD
///   total = 0.07025 USD
///   tokens: 1500 in + 2000 out + 500 cr + 2000 cw5 = 6000
fn opus_session(id: &str, day: &str) -> String {
    format!(
        concat!(
            r#"{{"type":"custom-title","customTitle":"named-{id}","sessionId":"{id}"}}"#,
            "\n",
            r#"{{"type":"user","sessionId":"{id}","entrypoint":"cli","timestamp":"{day}T10:00:00Z","cwd":"/tmp/proj-a","message":{{"role":"user","content":"hello"}}}}"#,
            "\n",
            r#"{{"type":"assistant","sessionId":"{id}","timestamp":"{day}T10:00:05Z","message":{{"role":"assistant","model":"claude-opus-4-7","content":[{{"type":"text","text":"hi"}}],"usage":{{"input_tokens":1000,"output_tokens":500,"cache_read_input_tokens":0,"cache_creation":{{"ephemeral_5m_input_tokens":2000,"ephemeral_1h_input_tokens":0}}}}}}}}"#,
            "\n",
            r#"{{"type":"user","sessionId":"{id}","timestamp":"{day}T10:01:00Z","message":{{"role":"user","content":"more"}}}}"#,
            "\n",
            r#"{{"type":"assistant","sessionId":"{id}","timestamp":"{day}T10:01:05Z","message":{{"role":"assistant","model":"claude-opus-4-7","content":[{{"type":"text","text":"ok"}}],"usage":{{"input_tokens":500,"output_tokens":1500,"cache_read_input_tokens":500,"cache_creation":{{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":0}}}}}}}}"#,
            "\n",
        ),
        id = id,
        day = day,
    )
}

/// One session JSONL on Sonnet 4.5, *un*named.
///
/// Sonnet ($3 in / $15 out):
///   msg: 2000 in + 1000 out = 0.006 + 0.015 = 0.021 USD
///   tokens: 3000
fn sonnet_session(id: &str, day: &str) -> String {
    format!(
        concat!(
            r#"{{"type":"user","sessionId":"{id}","entrypoint":"cli","timestamp":"{day}T12:00:00Z","cwd":"/tmp/proj-b","message":{{"role":"user","content":"hey"}}}}"#,
            "\n",
            r#"{{"type":"assistant","sessionId":"{id}","timestamp":"{day}T12:00:05Z","message":{{"role":"assistant","model":"claude-sonnet-4-5","content":[{{"type":"text","text":"yo"}}],"usage":{{"input_tokens":2000,"output_tokens":1000,"cache_read_input_tokens":0}}}}}}"#,
            "\n",
        ),
        id = id,
        day = day,
    )
}

#[test]
fn aggregate_three_sessions_across_two_projects() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let projects_dir = tmp.path().join("projects");
    let sessions_dir = tmp.path().join("sessions");
    fs::create_dir_all(&projects_dir).expect("mkdir projects");
    fs::create_dir_all(&sessions_dir).expect("mkdir sessions");

    // Project A — two Opus sessions both today.
    let proj_a = projects_dir.join("-tmp-proj-a");
    fs::create_dir_all(&proj_a).expect("mkdir proj-a");
    fs::write(
        proj_a.join("a-sess-001.jsonl"),
        opus_session("a-sess-001", "2026-04-16"),
    )
    .expect("write a1");
    fs::write(
        proj_a.join("a-sess-002.jsonl"),
        opus_session("a-sess-002", "2026-04-10"),
    )
    .expect("write a2");

    // Project B — one Sonnet session 45 days ago (outside the 30-day window).
    let proj_b = projects_dir.join("-tmp-proj-b");
    fs::create_dir_all(&proj_b).expect("mkdir proj-b");
    fs::write(
        proj_b.join("b-sess-001.jsonl"),
        sonnet_session("b-sess-001", "2026-03-02"),
    )
    .expect("write b1");

    // Also point session metadata to real cwds so the path resolver
    // returns human-friendly names.
    let resolved_a = tmp.path().join("proj-a");
    let resolved_b = tmp.path().join("proj-b");
    fs::create_dir_all(&resolved_a).expect("mkdir resolved-a");
    fs::create_dir_all(&resolved_b).expect("mkdir resolved-b");

    write_meta(&sessions_dir, "a-sess-001", &resolved_a);
    write_meta(&sessions_dir, "a-sess-002", &resolved_a);
    write_meta(&sessions_dir, "b-sess-001", &resolved_b);

    let today = NaiveDate::from_ymd_opt(2026, 4, 16).unwrap();
    let data = aggregate_from_dirs(&projects_dir, &sessions_dir, today).expect("aggregate ok");

    // ── Totals ──────────────────────────────────────────────────────────
    assert_eq!(data.totals.total_sessions, 3);
    assert_eq!(data.named_count, 2);
    assert_eq!(data.unnamed_count, 1);

    // 2 Opus sessions * (1500 in + 2000 out + 500 cr + 2000 cw5 = 6000 tok)
    // + 1 Sonnet session * 3000 tok = 15_000 tok
    assert_eq!(data.totals.total_tokens.total(), 15_000);
    assert_eq!(data.totals.total_tokens.input, 2 * 1500 + 2000);
    assert_eq!(data.totals.total_tokens.output, 2 * 2000 + 1000);
    assert_eq!(data.totals.total_tokens.cache_read, 2 * 500);
    assert_eq!(data.totals.total_tokens.cache_write_5m, 2 * 2000);
    assert_eq!(data.totals.total_tokens.cache_write_1h, 0);

    // Costs: 2 Opus * 0.07025 + 1 Sonnet * 0.021 = 0.16150
    let expected_cost = 2.0 * 0.070_25 + 0.021;
    assert!(
        (data.totals.total_cost_usd - expected_cost).abs() < 1e-6,
        "total_cost_usd {} vs {}",
        data.totals.total_cost_usd,
        expected_cost
    );

    // ── Per-project ─────────────────────────────────────────────────────
    // Two projects; proj-a (2 Opus) should outrank proj-b by cost.
    assert_eq!(data.by_project.len(), 2);
    let a = &data.by_project[0];
    let b = &data.by_project[1];
    assert_eq!(a.name, "proj-a");
    assert_eq!(a.session_count, 2);
    assert_eq!(a.total_tokens, 12_000);
    assert!((a.cost_usd - 2.0 * 0.070_25).abs() < 1e-6);
    assert_eq!(a.color_family, Family::Opus);

    assert_eq!(b.name, "proj-b");
    assert_eq!(b.session_count, 1);
    assert_eq!(b.total_tokens, 3_000);
    assert!((b.cost_usd - 0.021).abs() < 1e-6);
    assert_eq!(b.color_family, Family::Sonnet);

    // ── Daily window ────────────────────────────────────────────────────
    // Only the 2 Opus sessions are in the last 30 days. 4/10 and 4/16.
    // `build_stats_data` leaves this as the raw (unpadded) set.
    assert_eq!(data.daily.len(), 2);
    let mut dates: Vec<NaiveDate> = data.daily.iter().map(|d| d.date).collect();
    dates.sort();
    assert_eq!(
        dates,
        vec![
            NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
            NaiveDate::from_ymd_opt(2026, 4, 16).unwrap(),
        ]
    );

    // ── avg_cost_per_day = last-30d cost / 30 ──
    let last_30d_cost: f64 = data.daily.iter().map(|d| d.cost_usd).sum();
    assert!((data.totals.avg_cost_per_day - last_30d_cost / 30.0).abs() < 1e-9);
    // Specifically: only the two Opus sessions contribute.
    assert!((last_30d_cost - 2.0 * 0.070_25).abs() < 1e-6);

    // ── Per-model ───────────────────────────────────────────────────────
    assert_eq!(data.by_model.len(), 2);
    // Opus > Sonnet in cost, so Opus must sort first.
    assert_eq!(data.by_model[0].0, "claude-opus-4-7");
    assert_eq!(data.by_model[1].0, "claude-sonnet-4-5");
    assert!((data.by_model[0].1 - 2.0 * 0.070_25).abs() < 1e-6);
    assert!((data.by_model[1].1 - 0.021).abs() < 1e-6);
}

/// Helper: write a `~/.claude/sessions/<sid>.json` metadata file pointing
/// `sid` at `cwd`.
fn write_meta(sessions_dir: &Path, sid: &str, cwd: &Path) {
    let contents = format!(
        r#"{{"sessionId":"{sid}","cwd":{cwd:?}}}"#,
        sid = sid,
        cwd = cwd.to_string_lossy(),
    );
    fs::write(sessions_dir.join(format!("{sid}.json")), contents).expect("write meta");
}
