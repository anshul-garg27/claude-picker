//! `claude-picker stats` — Ratatui dashboard handler.
//!
//! End-to-end flow:
//!
//! 1. Walk `~/.claude/projects/*/*.jsonl` via the data layer.
//! 2. Aggregate per-project / per-day / per-model totals into a [`StatsData`].
//! 3. Take over the terminal (alt screen + raw + mouse), run an event loop
//!    that redraws on every tick and forwards key presses.
//! 4. Restore the terminal on exit — even if a panic fires inside the loop.
//!
//! The aggregation side lives here (not in `data/`) because it's specifically
//! the *dashboard* view of the data. The data layer exposes `Session` /
//! `Project`; turning those into per-day buckets is a presentation concern.
//!
//! Key bindings:
//! - `q`, `Esc`, `Ctrl+C` — quit.
//! - `e` — export every session to a CSV on the Desktop, show a toast.
//! - `t` — toggle timeline between "30 days" and "12 weeks".
//! - `r` — rescan `~/.claude/projects/` and rebuild the dashboard.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use chrono::{NaiveDate, Utc};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::data::claude_json_cache::ClaudeJsonCache;
use crate::data::pricing::{family, Family, TokenCounts};
use crate::data::session::{load_session_from_jsonl, Session};
use crate::data::{project, Project};
use crate::events::{self, Event};
use crate::theme::Theme;

use crate::ui::stats;
use stats::{
    build_daily_window, build_weekly_window, DailyStats, ProjectStats, StatsData, StatsView,
    TimelineMode, ToastKind, Totals,
};

// ── Entry point ──────────────────────────────────────────────────────────

pub fn run() -> anyhow::Result<()> {
    // Phase 0: try the fast path — read the per-project aggregates Claude
    // Code caches in `~/.claude.json` and build a rough StatsData from
    // them without parsing any JSONL. If every project is covered we can
    // skip Phase 1 entirely; otherwise we fall through.
    //
    // This lands the user on a dashboard in <100ms on large datasets.
    // The refresh key ("r") still kicks a full scan whenever they want
    // daily-window fidelity.
    let mut data = match try_aggregate_from_cache() {
        Some(d) => d,
        None => aggregate()?,
    };

    // Phase 2: UI loop.
    let mut terminal = setup_terminal()?;
    install_panic_hook();

    let result: anyhow::Result<()> = (|| {
        let theme = Theme::mocha();
        let mut mode = TimelineMode::Days30;
        let mut raw_daily = take_raw_daily(&data);
        // Replace `data.daily` with the 30-day window for first render.
        data.daily = build_daily_window(today(), &raw_daily, mode.buckets());

        let mut toast: Option<(String, ToastKind, Instant)> = None;
        let mut should_quit = false;
        // `?` help overlay toggle.
        let mut show_help = false;

        // Driver loop. `events::next()` blocks up to 50 ms, so toasts get a
        // free redraw tick without us needing a separate timer.
        while !should_quit {
            // Retire expired toasts before drawing so the frame is accurate.
            if toast
                .as_ref()
                .is_some_and(|(_, _, expires)| Instant::now() >= *expires)
            {
                toast = None;
            }

            let toast_msg = toast.as_ref().map(|(m, _, _)| m.clone());
            let toast_kind = toast
                .as_ref()
                .map(|(_, k, _)| *k)
                .unwrap_or(ToastKind::Info);

            terminal.draw(|f| {
                let view = StatsView {
                    data: &data,
                    mode,
                    toast: toast_msg.as_deref(),
                    toast_kind,
                };
                stats::render(f, f.area(), &view, &theme);
                if show_help {
                    let content =
                        crate::ui::help_overlay::help_for(crate::ui::help_overlay::Screen::Stats);
                    crate::ui::help_overlay::render(f, f.area(), content, &theme);
                }
            })?;

            let Some(ev) = events::next()? else {
                // No event in the poll window — go around for the next tick.
                continue;
            };

            // Help overlay steals input while visible.
            if show_help {
                match ev {
                    Event::Escape => show_help = false,
                    Event::Key(c) if crate::ui::help_overlay::is_dismiss_key(c) => {
                        show_help = false;
                    }
                    _ => {}
                }
                continue;
            }

            match ev {
                Event::Quit | Event::Ctrl('c') | Event::Escape => should_quit = true,
                Event::Key('q') => should_quit = true,
                Event::Key('?') => show_help = true,
                Event::Key('e') => match export_csv(&raw_daily, &data) {
                    Ok(path) => {
                        toast = Some((
                            format!("exported to {}", path.display()),
                            ToastKind::Success,
                            Instant::now() + Duration::from_millis(2000),
                        ));
                    }
                    Err(e) => {
                        toast = Some((
                            format!("export failed: {e}"),
                            ToastKind::Error,
                            Instant::now() + Duration::from_millis(2000),
                        ));
                    }
                },
                Event::Key('t') => {
                    mode = match mode {
                        TimelineMode::Days30 => TimelineMode::Weeks12,
                        TimelineMode::Weeks12 => TimelineMode::Days30,
                    };
                    data.daily = match mode {
                        TimelineMode::Days30 => {
                            build_daily_window(today(), &raw_daily, mode.buckets())
                        }
                        TimelineMode::Weeks12 => build_weekly_window(today(), &raw_daily),
                    };
                }
                Event::Key('r') => match aggregate() {
                    Ok(fresh) => {
                        data = fresh;
                        raw_daily = take_raw_daily(&data);
                        data.daily = match mode {
                            TimelineMode::Days30 => {
                                build_daily_window(today(), &raw_daily, mode.buckets())
                            }
                            TimelineMode::Weeks12 => build_weekly_window(today(), &raw_daily),
                        };
                        toast = Some((
                            "refreshed".to_string(),
                            ToastKind::Success,
                            Instant::now() + Duration::from_millis(1200),
                        ));
                    }
                    Err(e) => {
                        toast = Some((
                            format!("refresh failed: {e}"),
                            ToastKind::Error,
                            Instant::now() + Duration::from_millis(2000),
                        ));
                    }
                },
                _ => {}
            }
        }
        Ok(())
    })();

    let _ = restore_terminal(&mut terminal);
    result
}

/// Borrow-free helper — the dashboard swaps `data.daily` between the two
/// windowing modes, so we keep the unwindowed raw buckets out of band.
fn take_raw_daily(data: &StatsData) -> Vec<DailyStats> {
    data.daily.clone()
}

fn today() -> NaiveDate {
    Utc::now().date_naive()
}

// ── Aggregation ──────────────────────────────────────────────────────────

/// Cheap fast-path: build a `StatsData` from `~/.claude.json`'s per-project
/// cache without parsing any JSONL.
///
/// Returns `None` if the cache doesn't cover every discoverable project,
/// or if the cache's `lastSessionId` is stale (no longer the newest file
/// on disk for that project). In either case the caller should fall back
/// to the full [`aggregate`] scan.
///
/// The produced `StatsData` has an empty `daily` series — the cache does
/// not include per-session timestamps beyond the last one, so the timeline
/// chart will be empty until the user hits `r` to refresh.
pub fn try_aggregate_from_cache() -> Option<StatsData> {
    let home = dirs::home_dir()?;
    let projects_dir = home.join(".claude").join("projects");
    let sessions_meta_dir = home.join(".claude").join("sessions");
    let cache = ClaudeJsonCache::load_from(&home.join(".claude.json"));
    if cache.projects.is_empty() {
        return None;
    }
    let projects = project::discover_projects_in(&projects_dir, &sessions_meta_dir).ok()?;
    if projects.is_empty() {
        return None;
    }
    build_stats_data_from_cache(&projects, &cache, &projects_dir)
}

/// Unit-testable core of [`try_aggregate_from_cache`].
///
/// For each discovered project, check if the cache has an entry keyed on
/// the project's resolved cwd AND the cached `lastSessionId` matches the
/// newest JSONL on disk. If any project fails that check we bail and
/// return `None` — it's all-or-nothing, since a partial build would
/// silently under-count.
fn build_stats_data_from_cache(
    projects: &[Project],
    cache: &ClaudeJsonCache,
    projects_dir: &std::path::Path,
) -> Option<StatsData> {
    let mut totals = Totals::default();
    let mut by_project: Vec<ProjectStats> = Vec::with_capacity(projects.len());
    let mut by_model: HashMap<String, f64> = HashMap::new();

    for project in projects {
        let entry = cache.for_project(&project.path)?;
        let latest = newest_session_id_in_dir(&projects_dir.join(&project.encoded_dir));
        if !cache.is_fresh_for(&project.path, latest.as_deref()) {
            return None;
        }

        // Token/cost totals are the snapshot-for-last-session fields;
        // they're a per-project running counter for the newest session
        // only, but in the fast-path we treat them as the project total.
        // A full scan (via `r`) gives the precise multi-session picture.
        let tokens = TokenCounts {
            input: entry.last_total_input_tokens,
            output: entry.last_total_output_tokens,
            cache_read: entry.last_total_cache_read_input_tokens,
            cache_write_5m: entry.last_total_cache_creation_input_tokens,
            cache_write_1h: 0,
        };
        totals.total_tokens.add(tokens);
        totals.total_cost_usd += entry.last_cost;
        totals.total_sessions = totals.total_sessions.saturating_add(project.session_count);

        // Dominant model: extract the first key from `lastModelUsage` and
        // map it to a family. If the map is empty we fall back to Unknown
        // which renders as mauve.
        let dominant_model = entry
            .last_model_usage
            .as_object()
            .and_then(|m| m.keys().next())
            .cloned()
            .unwrap_or_default();
        let fam = if dominant_model.is_empty() {
            Family::Unknown
        } else {
            family(&dominant_model)
        };
        if !dominant_model.is_empty() {
            *by_model.entry(dominant_model).or_insert(0.0) += entry.last_cost;
        }

        by_project.push(ProjectStats {
            name: project.name.clone(),
            cost_usd: entry.last_cost,
            total_tokens: tokens.total(),
            session_count: project.session_count,
            color_family: fam,
        });
    }

    by_project.sort_by(|a, b| {
        b.cost_usd
            .partial_cmp(&a.cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut by_model_vec: Vec<(String, f64)> = by_model.into_iter().collect();
    by_model_vec.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    Some(StatsData {
        totals,
        by_project,
        daily: Vec::new(),
        by_model: by_model_vec,
        named_count: 0,
        unnamed_count: 0,
    })
}

/// Pick out the session id (jsonl basename) with the newest mtime in a
/// project directory. Returns `None` if the directory is missing or holds
/// no jsonl.
fn newest_session_id_in_dir(dir: &std::path::Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut best: Option<(std::time::SystemTime, String)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str())?.to_string();
        let mtime = entry.metadata().ok()?.modified().ok()?;
        match &best {
            Some((t, _)) if *t >= mtime => {}
            _ => best = Some((mtime, stem)),
        }
    }
    best.map(|(_, id)| id)
}

/// Scan every project + session under `~/.claude/projects/` and return a
/// fully populated [`StatsData`].
pub fn aggregate() -> anyhow::Result<StatsData> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    let projects_dir = home.join(".claude").join("projects");
    let sessions_meta_dir = home.join(".claude").join("sessions");
    aggregate_from_dirs(&projects_dir, &sessions_meta_dir, today())
}

/// Test-friendly variant that takes explicit roots and "today".
pub fn aggregate_from_dirs(
    projects_dir: &std::path::Path,
    sessions_meta_dir: &std::path::Path,
    today_date: NaiveDate,
) -> anyhow::Result<StatsData> {
    let projects = project::discover_projects_in(projects_dir, sessions_meta_dir)?;
    let mut all_sessions: Vec<(Project, Vec<Session>)> = Vec::with_capacity(projects.len());

    for project in projects {
        let dir = projects_dir.join(&project.encoded_dir);
        let mut project_sessions = Vec::new();
        if dir.is_dir() {
            for entry in std::fs::read_dir(&dir)? {
                let Ok(entry) = entry else { continue };
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                    continue;
                }
                match load_session_from_jsonl(&path, project.path.clone()) {
                    Ok(Some(s)) => project_sessions.push(s),
                    Ok(None) => {}
                    Err(e) => eprintln!("{}: load error: {e}", path.display()),
                }
            }
        }
        all_sessions.push((project, project_sessions));
    }

    Ok(build_stats_data(&all_sessions, today_date))
}

/// Pure aggregation function — separate from `aggregate` so tests can hand
/// it a synthetic set of sessions.
pub fn build_stats_data(projects: &[(Project, Vec<Session>)], today_date: NaiveDate) -> StatsData {
    // ── Lifetime totals ──────────────────────────────────────────────────
    let mut totals = Totals::default();
    let mut named_count = 0u32;
    let mut unnamed_count = 0u32;
    let mut by_model: HashMap<String, f64> = HashMap::new();

    // Per-project accumulators keyed by project name.
    let mut by_project: HashMap<String, ProjectStats> = HashMap::new();

    // Per-day accumulators keyed by the session's last-timestamp date.
    let mut daily: HashMap<NaiveDate, DailyStats> = HashMap::new();

    for (project, sessions) in projects {
        for s in sessions {
            totals.total_tokens.add(s.tokens);
            totals.total_cost_usd += s.total_cost_usd;
            totals.total_sessions = totals.total_sessions.saturating_add(1);
            if s.name.is_some() {
                named_count += 1;
            } else {
                unnamed_count += 1;
            }

            if !s.model_summary.is_empty() {
                *by_model.entry(s.model_summary.clone()).or_insert(0.0) += s.total_cost_usd;
            }

            let entry = by_project
                .entry(project.name.clone())
                .or_insert_with(|| ProjectStats {
                    name: project.name.clone(),
                    cost_usd: 0.0,
                    total_tokens: 0,
                    session_count: 0,
                    color_family: family(&s.model_summary),
                });
            entry.cost_usd += s.total_cost_usd;
            entry.total_tokens = entry.total_tokens.saturating_add(s.tokens.total());
            entry.session_count = entry.session_count.saturating_add(1);
            // Prefer a non-Unknown family if we saw one.
            if entry.color_family == Family::Unknown {
                entry.color_family = family(&s.model_summary);
            }

            // Daily bucket — only sessions in the last 30 days contribute.
            if let Some(ts) = s.last_timestamp {
                let d = ts.date_naive();
                let age = today_date.signed_duration_since(d).num_days();
                if (0..=29).contains(&age) {
                    let bucket = daily.entry(d).or_insert(DailyStats {
                        date: d,
                        sessions: 0,
                        tokens: 0,
                        cost_usd: 0.0,
                    });
                    bucket.sessions = bucket.sessions.saturating_add(1);
                    bucket.tokens = bucket.tokens.saturating_add(s.tokens.total());
                    bucket.cost_usd += s.total_cost_usd;
                }
            }
        }
    }

    // Mean cost over the last 30 days (for the KPI subtitle).
    let last_30_cost: f64 = daily.values().map(|d| d.cost_usd).sum();
    totals.avg_cost_per_day = last_30_cost / 30.0;

    // Flatten + sort the per-project block by cost descending, cap at 8.
    let mut by_project_vec: Vec<ProjectStats> = by_project.into_values().collect();
    by_project_vec.sort_by(|a, b| {
        b.cost_usd
            .partial_cmp(&a.cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Flatten per-model, sort by cost descending.
    let mut by_model_vec: Vec<(String, f64)> = by_model.into_iter().collect();
    by_model_vec.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Flatten daily — unsorted, the caller will window/reorder it.
    let daily_vec: Vec<DailyStats> = daily.into_values().collect();

    StatsData {
        totals,
        by_project: by_project_vec,
        daily: daily_vec,
        by_model: by_model_vec,
        named_count,
        unnamed_count,
    }
}

// ── CSV export ───────────────────────────────────────────────────────────

/// Write a CSV of per-day sessions/tokens/cost to `~/Desktop/claude-picker-stats-<date>.csv`.
///
/// Returns the written path.
fn export_csv(raw_daily: &[DailyStats], data: &StatsData) -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    let desktop = home.join("Desktop");
    let target_dir = if desktop.is_dir() { desktop } else { home };
    let today_str = today().format("%Y-%m-%d").to_string();
    let path = target_dir.join(format!("claude-picker-stats-{today_str}.csv"));

    let mut out = String::with_capacity(4096);
    out.push_str("section,key,sessions,tokens,cost_usd\n");

    // Totals row.
    out.push_str(&format!(
        "totals,all,{},{},{:.4}\n",
        data.totals.total_sessions,
        data.totals.total_tokens.total(),
        data.totals.total_cost_usd,
    ));

    // Per-project rows.
    for p in &data.by_project {
        out.push_str(&format!(
            "project,{},{},{},{:.4}\n",
            csv_escape(&p.name),
            p.session_count,
            p.total_tokens,
            p.cost_usd,
        ));
    }

    // Per-day rows (use the pre-windowed raw set so ordering is stable).
    let mut sorted = raw_daily.to_vec();
    sorted.sort_by_key(|d| d.date);
    for d in &sorted {
        out.push_str(&format!(
            "day,{},{},{},{:.4}\n",
            d.date, d.sessions, d.tokens, d.cost_usd,
        ));
    }

    // Per-model rows.
    for (model, cost) in &data.by_model {
        out.push_str(&format!("model,{},0,0,{:.4}\n", csv_escape(model), cost,));
    }

    fs::write(&path, out)?;
    Ok(path)
}

/// Minimal RFC 4180 escaping — quote any field that contains `"`, `,`, or a
/// newline. We control the other columns so they don't need escaping.
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        let escaped = s.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        s.to_string()
    }
}

// ── Terminal lifecycle ───────────────────────────────────────────────────

fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let mut stdout = io::stdout();
        let _ = disable_raw_mode();
        let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
        default(info);
    }));
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::pricing::TokenCounts;
    use crate::data::session::SessionKind;
    use std::path::PathBuf;

    fn mk_project(name: &str) -> Project {
        Project {
            name: name.to_string(),
            path: PathBuf::from(format!("/tmp/{name}")),
            encoded_dir: format!("-tmp-{name}"),
            session_count: 0,
            last_activity: None,
            git_branch: None,
        }
    }

    fn mk_session(
        id: &str,
        project_dir: &std::path::Path,
        cost: f64,
        tokens: TokenCounts,
        day: NaiveDate,
        model: &str,
        named: bool,
    ) -> Session {
        let ts = day.and_hms_opt(12, 0, 0).unwrap().and_utc();
        Session {
            id: id.to_string(),
            project_dir: project_dir.to_path_buf(),
            name: if named {
                Some("named".to_string())
            } else {
                None
            },
            auto_name: Some(id.to_string()),
            last_prompt: None,
            message_count: 4,
            tokens,
            total_cost_usd: cost,
            model_summary: model.to_string(),
            first_timestamp: Some(ts),
            last_timestamp: Some(ts),
            is_fork: false,
            forked_from: None,
            entrypoint: SessionKind::Cli,
            permission_mode: None,
            subagent_count: 0,
        }
    }

    #[test]
    fn build_stats_data_aggregates_totals_by_project_and_day() {
        let today = NaiveDate::from_ymd_opt(2026, 4, 16).unwrap();

        let alpha = mk_project("alpha");
        let beta = mk_project("beta");

        let tokens = TokenCounts {
            input: 1_000,
            output: 500,
            cache_read: 250,
            cache_write_5m: 500,
            cache_write_1h: 0,
        };

        let sessions_alpha = vec![
            mk_session(
                "a1",
                &alpha.path,
                1.23,
                tokens,
                today,
                "claude-opus-4-7",
                true,
            ),
            mk_session(
                "a2",
                &alpha.path,
                0.77,
                tokens,
                today - chrono::Duration::days(2),
                "claude-opus-4-7",
                false,
            ),
        ];
        let sessions_beta = vec![mk_session(
            "b1",
            &beta.path,
            0.50,
            tokens,
            today - chrono::Duration::days(10),
            "claude-sonnet-4-5",
            false,
        )];

        let data = build_stats_data(
            &[
                (alpha.clone(), sessions_alpha),
                (beta.clone(), sessions_beta),
            ],
            today,
        );

        // Totals.
        assert_eq!(data.totals.total_sessions, 3);
        let expected_tokens = tokens.total() * 3;
        assert_eq!(data.totals.total_tokens.total(), expected_tokens);
        let expected_cost = 1.23 + 0.77 + 0.50;
        assert!(
            (data.totals.total_cost_usd - expected_cost).abs() < 1e-9,
            "total_cost_usd mismatch: got {}",
            data.totals.total_cost_usd
        );

        // Named / unnamed split.
        assert_eq!(data.named_count, 1);
        assert_eq!(data.unnamed_count, 2);

        // Per-project sort by cost desc: alpha > beta.
        assert_eq!(data.by_project.len(), 2);
        assert_eq!(data.by_project[0].name, "alpha");
        assert!((data.by_project[0].cost_usd - 2.0).abs() < 1e-9);
        assert_eq!(data.by_project[0].session_count, 2);
        assert_eq!(data.by_project[1].name, "beta");
        assert_eq!(data.by_project[1].session_count, 1);
        assert_eq!(data.by_project[0].color_family, Family::Opus);
        assert_eq!(data.by_project[1].color_family, Family::Sonnet);

        // Daily: 3 buckets (today, today-2, today-10).
        assert_eq!(data.daily.len(), 3);

        // Per-model.
        assert_eq!(data.by_model.len(), 2);
        assert_eq!(data.by_model[0].0, "claude-opus-4-7");
        assert!((data.by_model[0].1 - 2.0).abs() < 1e-9);

        // avg_cost_per_day = total cost in 30d / 30.
        // All 3 sessions are inside the window, so it's total_cost / 30.
        assert!((data.totals.avg_cost_per_day - expected_cost / 30.0).abs() < 1e-9);
    }

    #[test]
    fn old_sessions_contribute_totals_but_not_daily() {
        let today = NaiveDate::from_ymd_opt(2026, 4, 16).unwrap();
        let old_day = today - chrono::Duration::days(45);
        let project = mk_project("oldie");
        let tokens = TokenCounts {
            input: 500,
            output: 100,
            ..Default::default()
        };
        let sessions = vec![mk_session(
            "old1",
            &project.path,
            5.0,
            tokens,
            old_day,
            "claude-opus-4-7",
            true,
        )];
        let data = build_stats_data(&[(project, sessions)], today);
        assert_eq!(data.totals.total_sessions, 1);
        assert!((data.totals.total_cost_usd - 5.0).abs() < 1e-9);
        // But the 30-day daily window is empty.
        assert!(data.daily.is_empty());
        // And avg_cost_per_day is zero (no spend inside the window).
        assert!((data.totals.avg_cost_per_day - 0.0).abs() < 1e-9);
    }

    #[test]
    fn csv_escape_quotes_commas_and_quotes() {
        assert_eq!(csv_escape("simple"), "simple");
        assert_eq!(csv_escape("with, comma"), "\"with, comma\"");
        assert_eq!(csv_escape("with \"quote\""), "\"with \"\"quote\"\"\"");
    }

    #[test]
    fn build_stats_data_from_cache_uses_cache_when_fresh() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let projects_dir = tmp.path().join("projects");
        let encoded = "-Users-me-alpha";
        let proj_dir = projects_dir.join(encoded);
        std::fs::create_dir_all(&proj_dir).expect("mkdir");
        // Two sessions, mtime of the newer one is the `lastSessionId`.
        let old = proj_dir.join("old.jsonl");
        std::fs::write(&old, b"").expect("write");
        // Sleep a microsecond by bumping the newer file's mtime. In
        // practice both files inherit the current mtime; we rely on write
        // order so the second write is strictly newer on POSIX.
        std::thread::sleep(std::time::Duration::from_millis(5));
        let new = proj_dir.join("newsid.jsonl");
        std::fs::write(&new, b"").expect("write");

        let claude_json = tmp.path().join(".claude.json");
        std::fs::write(
            &claude_json,
            r#"{
              "projects": {
                "/tmp/alpha": {
                  "lastCost": 2.50,
                  "lastTotalInputTokens": 1000,
                  "lastTotalOutputTokens": 500,
                  "lastSessionId": "newsid",
                  "lastModelUsage": {"claude-opus-4-7": {"tokens": 1500}}
                }
              }
            }"#,
        )
        .expect("write claude.json");

        let cache = ClaudeJsonCache::load_from(&claude_json);
        let project = Project {
            name: "alpha".into(),
            path: PathBuf::from("/tmp/alpha"),
            encoded_dir: encoded.to_string(),
            session_count: 2,
            last_activity: None,
            git_branch: None,
        };
        let data = build_stats_data_from_cache(&[project], &cache, &projects_dir)
            .expect("cache fast-path must succeed");
        assert_eq!(data.by_project.len(), 1);
        assert_eq!(data.by_project[0].name, "alpha");
        assert!((data.by_project[0].cost_usd - 2.50).abs() < 1e-9);
        assert_eq!(data.by_project[0].total_tokens, 1500);
        assert_eq!(data.by_project[0].color_family, Family::Opus);
        // 30-day series is always empty on the cache fast-path.
        assert!(data.daily.is_empty());
    }

    #[test]
    fn build_stats_data_from_cache_bails_when_stale() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let projects_dir = tmp.path().join("projects");
        let encoded = "-Users-me-beta";
        let proj_dir = projects_dir.join(encoded);
        std::fs::create_dir_all(&proj_dir).expect("mkdir");
        std::fs::write(proj_dir.join("currsid.jsonl"), b"").expect("write");
        let claude_json = tmp.path().join(".claude.json");
        std::fs::write(
            &claude_json,
            r#"{"projects":{"/tmp/beta":{"lastSessionId":"oldsid","lastCost":1.0}}}"#,
        )
        .expect("write");
        let cache = ClaudeJsonCache::load_from(&claude_json);
        let project = Project {
            name: "beta".into(),
            path: PathBuf::from("/tmp/beta"),
            encoded_dir: encoded.to_string(),
            session_count: 1,
            last_activity: None,
            git_branch: None,
        };
        let data = build_stats_data_from_cache(&[project], &cache, &projects_dir);
        assert!(
            data.is_none(),
            "stale cache must trigger full-scan fallback"
        );
    }

    #[test]
    fn newest_session_id_picks_mtime_winner() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("a.jsonl"), b"").expect("write a");
        std::thread::sleep(std::time::Duration::from_millis(5));
        std::fs::write(tmp.path().join("b.jsonl"), b"").expect("write b");
        let id = newest_session_id_in_dir(tmp.path()).expect("some");
        assert_eq!(id, "b");
    }
}
