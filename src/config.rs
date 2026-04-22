//! TOML configuration file at `~/.config/claude-picker/config.toml`.
//!
//! The config is the THIRD source in the precedence chain for every option:
//!
//!   1. CLI flag (e.g. `--theme dracula`)   — highest priority
//!   2. Env var (e.g. `CLAUDE_PICKER_THEME=dracula`)
//!   3. This config file
//!   4. Built-in default
//!
//! A missing file is NOT an error — every field has a sensible default, and
//! [`Config::load`] falls back to `Config::default()` when the on-disk copy
//! doesn't exist or isn't readable. This keeps first-run friction zero.
//!
//! Malformed TOML IS surfaced as an error: we want users to know their
//! `config.toml` was rejected rather than silently falling back to defaults
//! and leaving them confused about why their new theme didn't stick.
//!
//! The `--generate-config` CLI flag writes a fully-commented template to the
//! default location so users have a starting point. With `--force` it
//! overwrites an existing file; without, the command aborts if one exists.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Top-level configuration. All fields are defaulted via `#[serde(default)]`
/// so partial files (just `[ui]` with nothing else, for example) still parse
/// successfully.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub picker: PickerConfig,
    #[serde(default)]
    pub actions: ActionsConfig,
    #[serde(default)]
    pub keys: KeysConfig,
    #[serde(default)]
    pub bookmarks: BookmarksConfig,
}

/// `[ui]` — appearance + global display toggles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// One of: kanagawa, finance-terminal, parchment-dark, paperwhite-warm,
    /// catppuccin-mocha, catppuccin-latte, dracula, tokyo-night, gruvbox-dark,
    /// nord, nord-aurora, rose-pine-moon, high-contrast, colorblind-safe.
    /// Unknown values fall through to the next source.
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Override the relative-time subtitle. Empty = smart default. Uses
    /// strftime syntax when non-empty (reserved for a future build — the
    /// v2.2 picker still uses the smart format).
    #[serde(default)]
    pub date_format: String,

    /// Column cap on the `--stats` dashboard. 0 means "use full terminal".
    #[serde(default)]
    pub stats_width: u16,

    /// Skip every opt-in animation — the F1 fork-graph radial expansion,
    /// the F3 pulse-HUD beat, the F4 replay comet trail, and the F5 peek
    /// slide-in. The one-shot toast slide + cursor-glide trail are
    /// already governed by `CLAUDE_PICKER_NO_ANIM`; this flag is the
    /// tachyonfx-era follow-up that moves the preference into config.
    ///
    /// Default `false` so the animations are on by default on modern
    /// terminals. Users on SSH / screen readers / prefers-reduced-motion
    /// flip this to `true` in `~/.config/claude-picker/config.toml`.
    #[serde(default)]
    pub reduce_motion: bool,

    /// Paint a subtle `surface0`-wash stripe on every other session row so
    /// the tabular lists read with zebra contrast. Only applies to dark
    /// themes where `base` and `surface0` differ; light themes with
    /// minimal base↔surface0 delta skip the stripe so the palette inversion
    /// doesn't flip contrast.
    ///
    /// Default `true`. Users who find the striping noisy can disable it in
    /// `~/.config/claude-picker/config.toml`:
    ///
    /// ```toml
    /// [ui]
    /// zebra_rows = false
    /// ```
    #[serde(default = "default_zebra_rows")]
    pub zebra_rows: bool,

    /// Subscription tier used by the `stats` quota panel (feature #22). One
    /// of:
    ///
    /// - `"none"` (default) — no panel rendered.
    /// - `"pro"` — $20/mo cap.
    /// - `"max"` — $100/mo cap.
    /// - `"max20"` — $200/mo cap.
    /// - `"team"` — $30/user/mo cap.
    /// - `"enterprise"` — no cap rendered (panel shows plan only).
    ///
    /// The caps are best-effort estimates — users on a negotiated plan can
    /// override them by setting a custom monthly budget via `b` on the stats
    /// dashboard.
    #[serde(default = "default_plan_tier")]
    pub plan_tier: String,

    /// Auto-redact well-known secret shapes (API keys, tokens, JWTs, bearer
    /// headers) whenever message text is rendered in the preview pane or the
    /// full-screen conversation viewer. Matches against known prefixes
    /// (`sk-ant-…`, `sk-proj-…`, `AKIA…`, `ASIA…`, `ghp_…`, `gho_…`, `ghu_…`,
    /// `ghs_…`, `eyJ….….…`, `Bearer …`) and replaces the sensitive tail with
    /// `****<last4>` so snapshots, demos, and screenshots never accidentally
    /// leak a working credential.
    ///
    /// Default `true`. Users who need to see the raw content (e.g. debugging
    /// a token themselves) flip this to `false` in
    /// `~/.config/claude-picker/config.toml`:
    ///
    /// ```toml
    /// [ui]
    /// redact_preview = false
    /// ```
    #[serde(default = "default_redact_preview")]
    pub redact_preview: bool,
}

fn default_zebra_rows() -> bool {
    true
}

fn default_plan_tier() -> String {
    "none".to_string()
}

fn default_redact_preview() -> bool {
    true
}

/// Plan-tier metadata for the `stats` quota panel. `None` = panel hidden;
/// `Some((label, Some(cap)))` renders a progress bar against the cap;
/// `Some((label, None))` renders a "no cap" line (enterprise).
pub fn plan_tier_info(tier: &str) -> Option<(&'static str, Option<f64>)> {
    match tier.trim().to_ascii_lowercase().as_str() {
        "pro" => Some(("Pro", Some(20.0))),
        "max" => Some(("Max", Some(100.0))),
        "max20" => Some(("Max 20\u{00D7}", Some(200.0))),
        "team" => Some(("Team", Some(30.0))),
        "enterprise" => Some(("Enterprise", None)),
        _ => None,
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            date_format: String::new(),
            stats_width: 0,
            reduce_motion: false,
            zebra_rows: default_zebra_rows(),
            plan_tier: default_plan_tier(),
            redact_preview: default_redact_preview(),
        }
    }
}

fn default_theme() -> String {
    "kanagawa".to_string()
}

/// `[picker]` — what gets shown in the default picker screen and how.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickerConfig {
    /// Default sort: one of "recent", "cost", "msgs", "name", "bookmarked-first".
    #[serde(default = "default_sort")]
    pub sort: String,

    /// Whether to surface projects whose name starts with ".". Default true so
    /// `.claude-picker` and similar dot-dirs keep appearing; v2.1 behaviour.
    #[serde(default = "default_include_hidden")]
    pub include_hidden_projects: bool,

    /// Sessions with fewer than this many messages are skipped. Default 2
    /// matches the current hard-coded filter in the session loader.
    #[serde(default = "default_min_messages")]
    pub min_messages: u32,

    /// Filter to a single family: "", "opus", "sonnet", "haiku". Empty = no
    /// filter.
    #[serde(default)]
    pub model_filter: String,
}

impl Default for PickerConfig {
    fn default() -> Self {
        Self {
            sort: default_sort(),
            include_hidden_projects: default_include_hidden(),
            min_messages: default_min_messages(),
            model_filter: String::new(),
        }
    }
}

fn default_sort() -> String {
    "bookmarked-first".to_string()
}
fn default_include_hidden() -> bool {
    true
}
fn default_min_messages() -> u32 {
    2
}

/// `[actions]` — external-process options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionsConfig {
    /// Flags forwarded to `claude --resume`. `CLAUDE_PICKER_FLAGS` env var
    /// still wins when set; this is the fallback.
    #[serde(default = "default_claude_flags")]
    pub claude_flags: String,

    /// Override for the `o` keybinding's editor. Empty = use the chain
    /// `$EDITOR → code → cursor → nvim → vim`.
    #[serde(default)]
    pub editor: String,
}

impl Default for ActionsConfig {
    fn default() -> Self {
        Self {
            claude_flags: default_claude_flags(),
            editor: String::new(),
        }
    }
}

fn default_claude_flags() -> String {
    "--dangerously-skip-permissions".to_string()
}

/// `[keys]` — per-action keybinding overrides. Every field is `Option<String>`
/// so omitted entries stay as defaults; a user who sets just `bookmark` keeps
/// every other binding intact.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeysConfig {
    #[serde(default)]
    pub bookmark: Option<String>,
    #[serde(default)]
    pub export: Option<String>,
    #[serde(default)]
    pub delete: Option<String>,
    #[serde(default)]
    pub rename: Option<String>,
    #[serde(default)]
    pub copy_id: Option<String>,
    #[serde(default)]
    pub copy_path: Option<String>,
    #[serde(default)]
    pub open_editor: Option<String>,
}

/// `[bookmarks]` — where the on-disk bookmark store lives.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BookmarksConfig {
    /// Custom path. Empty = the v1 default of `~/.claude-picker/bookmarks.json`.
    #[serde(default)]
    pub path: String,
}

// ── Load / save / template ────────────────────────────────────────────────

impl Config {
    /// Default location: `~/.config/claude-picker/config.toml`. Returns `None`
    /// when the home dir can't be located (headless CI containers etc.).
    pub fn default_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".config").join("claude-picker").join("config.toml"))
    }

    /// Load the config from its default on-disk location, returning built-in
    /// defaults when the file is missing. Malformed TOML surfaces as an error
    /// so the user knows their edits were rejected.
    pub fn load() -> Result<Self> {
        match Self::default_path() {
            Some(path) => Self::load_from(&path),
            None => Ok(Self::default()),
        }
    }

    /// Load from an explicit path. Missing file → defaults; malformed TOML →
    /// error with context.
    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path)
            .with_context(|| format!("reading config at {}", path.display()))?;
        let cfg: Self =
            toml::from_str(&raw).with_context(|| format!("parsing TOML at {}", path.display()))?;
        Ok(cfg)
    }

    /// Write the fully-commented default template to `path`, creating the
    /// parent directory if needed. Refuses to overwrite an existing file
    /// unless `force` is true.
    pub fn write_template(path: &Path, force: bool) -> Result<()> {
        if path.exists() && !force {
            anyhow::bail!(
                "{} already exists; pass --force to overwrite",
                path.display()
            );
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating config parent dir {}", parent.display()))?;
        }
        fs::write(path, DEFAULT_TEMPLATE)
            .with_context(|| format!("writing config to {}", path.display()))?;
        Ok(())
    }
}

/// Fully-commented template. Kept in sync with the field defaults above.
///
/// We ship a hand-written template rather than serde-serializing `Config`
/// because users want the leading comments — TOML's serde emitter drops
/// anything that isn't a value. The trade-off is that new fields must be
/// added here AND in the struct; the round-trip test enforces the struct
/// side stays parseable.
pub const DEFAULT_TEMPLATE: &str = r#"# claude-picker configuration
# Generate this file with `claude-picker --generate-config`.
#
# Precedence (highest wins):
#   1. CLI flag (e.g. `--theme dracula`)
#   2. Env var (e.g. `CLAUDE_PICKER_THEME=dracula`)
#   3. This file
#   4. Built-in default

[ui]
# Default theme. One of: kanagawa (default), finance-terminal, parchment-dark,
# paperwhite-warm, catppuccin-mocha, catppuccin-latte, dracula, tokyo-night,
# gruvbox-dark, nord, nord-aurora, rose-pine-moon, high-contrast, colorblind-safe.
theme = "kanagawa"

# When non-empty, overrides the subtitle timestamp format. Default uses
# smart relative time (5m, 2h, yesterday, Apr 12). Use strftime syntax.
date_format = ""

# Width cap for the --stats dashboard in columns. 0 = use full terminal.
stats_width = 0

# Skip every opt-in animation: the fork-graph radial expansion, the
# session HUD pulse, the replay scrubber comet trail, and the peek-mode
# slide-in. Leave false to enjoy them; set true for SSH / screen readers
# / prefers-reduced-motion setups. The `CLAUDE_PICKER_NO_ANIM=1` env var
# still works for the pre-tachyonfx effects (toast slide, cursor glide).
reduce_motion = false

# Paint a subtle surface0-wash stripe on every other session row so the
# tabular lists read with zebra contrast. Only applies to dark themes
# where base and surface0 differ meaningfully; light themes skip the
# stripe automatically so the palette inversion doesn't flip contrast.
# Default true. Flip to false for a flat, fully-uniform list.
zebra_rows = true

# Subscription tier for the stats quota panel (feature #22). One of:
#   "none"       — hide the quota panel (default)
#   "pro"        — $20/mo cap
#   "max"        — $100/mo cap
#   "max20"      — $200/mo cap
#   "team"       — $30/user/mo cap
#   "enterprise" — no cap shown
# The caps are best-effort estimates; users on negotiated plans can
# override with the 'b' budget modal on the stats dashboard.
plan_tier = "none"

# Auto-redact well-known secret shapes (API keys, tokens, JWTs, bearer
# headers) whenever message text is rendered in the preview pane or the
# conversation viewer. Matched tokens are replaced with `****<last4>`.
# Default true. Flip to false to show raw content when debugging.
redact_preview = true

[picker]
# Default sort for the session list. One of: recent, cost, msgs, name,
# bookmarked-first.
sort = "bookmarked-first"

# Include .hidden projects (name starting with "."). Default true
# matches v2 behavior (e.g. .claude-picker shows up).
include_hidden_projects = true

# Skip sessions with fewer than this many messages from the picker list.
# Default 2 (matches current filter).
min_messages = 2

# Filter sessions to one model family. Empty = show all. One of: "",
# "opus", "sonnet", "haiku".
model_filter = ""

[actions]
# Default flags passed to `claude --resume`. Matches
# CLAUDE_PICKER_FLAGS env behavior (env wins if set).
claude_flags = "--dangerously-skip-permissions"

# Editor to launch via `o` key. Falls back to $EDITOR, then code,
# cursor, nvim, vim. Leave empty to use the fallback chain.
editor = ""

[keys]
# Rebind specific actions. Keys are action IDs; values are single keys
# or key chords. Omitted entries use the default.
#   bookmark = "Ctrl+B"
#   export = "Ctrl+E"
#   delete = "Ctrl+D"
#   rename = "r"
#   copy_id = "y"
#   copy_path = "Y"
#   open_editor = "o"

[bookmarks]
# Where bookmarks are persisted. Default ~/.claude-picker/bookmarks.json
path = ""
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn defaults_are_sane() {
        let c = Config::default();
        assert_eq!(c.ui.theme, "kanagawa");
        assert!(!c.ui.reduce_motion, "animations should be on by default");
        assert!(c.ui.zebra_rows, "zebra striping should be on by default");
        assert_eq!(c.ui.plan_tier, "none");
        assert!(
            c.ui.redact_preview,
            "preview secret-redaction must be on by default",
        );
        assert_eq!(c.picker.sort, "bookmarked-first");
        assert_eq!(c.picker.min_messages, 2);
        assert!(c.picker.include_hidden_projects);
        assert_eq!(c.picker.model_filter, "");
        assert_eq!(c.actions.claude_flags, "--dangerously-skip-permissions");
        assert_eq!(c.actions.editor, "");
        assert_eq!(c.bookmarks.path, "");
    }

    #[test]
    fn missing_file_returns_defaults() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("missing.toml");
        let cfg = Config::load_from(&path).expect("missing is OK");
        // Should equal defaults — compare a couple of marker fields.
        assert_eq!(cfg.ui.theme, Config::default().ui.theme);
        assert_eq!(cfg.picker.sort, Config::default().picker.sort);
    }

    #[test]
    fn round_trip_default_template_parses() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("config.toml");
        Config::write_template(&path, false).expect("write");
        let cfg = Config::load_from(&path).expect("parse");

        // Every field in the template must equal the matching default.
        let d = Config::default();
        assert_eq!(cfg.ui.theme, d.ui.theme);
        assert_eq!(cfg.ui.date_format, d.ui.date_format);
        assert_eq!(cfg.ui.stats_width, d.ui.stats_width);
        assert_eq!(cfg.ui.reduce_motion, d.ui.reduce_motion);
        assert_eq!(cfg.ui.zebra_rows, d.ui.zebra_rows);
        assert_eq!(cfg.ui.plan_tier, d.ui.plan_tier);
        assert_eq!(cfg.ui.redact_preview, d.ui.redact_preview);
        assert_eq!(cfg.picker.sort, d.picker.sort);
        assert_eq!(
            cfg.picker.include_hidden_projects,
            d.picker.include_hidden_projects
        );
        assert_eq!(cfg.picker.min_messages, d.picker.min_messages);
        assert_eq!(cfg.picker.model_filter, d.picker.model_filter);
        assert_eq!(cfg.actions.claude_flags, d.actions.claude_flags);
        assert_eq!(cfg.actions.editor, d.actions.editor);
        assert_eq!(cfg.bookmarks.path, d.bookmarks.path);
    }

    #[test]
    fn partial_file_merges_with_defaults() {
        // Only [ui] set — picker section should still carry defaults.
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("config.toml");
        fs::write(&path, "[ui]\ntheme = \"dracula\"\n").expect("write");
        let cfg = Config::load_from(&path).expect("parse");
        assert_eq!(cfg.ui.theme, "dracula");
        assert_eq!(cfg.picker.sort, "bookmarked-first");
        assert_eq!(cfg.picker.min_messages, 2);
    }

    #[test]
    fn malformed_toml_errors_clearly() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("config.toml");
        fs::write(&path, "[ui]\ntheme = not-a-string\n").expect("write");
        let err = Config::load_from(&path).expect_err("should fail");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("parsing TOML"),
            "error should mention parsing, got: {msg}",
        );
    }

    #[test]
    fn write_template_refuses_to_overwrite_without_force() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("config.toml");
        Config::write_template(&path, false).expect("first write");
        let err = Config::write_template(&path, false).expect_err("should refuse");
        assert!(
            format!("{err:#}").contains("already exists"),
            "error should explain",
        );
    }

    #[test]
    fn write_template_force_overwrites() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("config.toml");
        fs::write(&path, "old content").expect("seed");
        Config::write_template(&path, true).expect("force write");
        let raw = fs::read_to_string(&path).expect("read");
        assert!(raw.contains("claude-picker configuration"));
        assert!(!raw.contains("old content"));
    }

    #[test]
    fn write_template_creates_parent_dir() {
        let tmp = tempdir().expect("tempdir");
        let nested = tmp.path().join("a").join("b").join("config.toml");
        assert!(!nested.parent().unwrap().exists());
        Config::write_template(&nested, false).expect("should create dirs");
        assert!(nested.is_file());
    }

    #[test]
    fn plan_tier_info_maps_known_tiers() {
        let (label, cap) = plan_tier_info("pro").expect("pro recognized");
        assert_eq!(label, "Pro");
        assert_eq!(cap, Some(20.0));
        let (label, cap) = plan_tier_info("max").expect("max recognized");
        assert_eq!(label, "Max");
        assert_eq!(cap, Some(100.0));
        let (_, cap) = plan_tier_info("max20").expect("max20 recognized");
        assert_eq!(cap, Some(200.0));
        let (_, cap) = plan_tier_info("team").expect("team recognized");
        assert_eq!(cap, Some(30.0));
        let (label, cap) = plan_tier_info("enterprise").expect("enterprise recognized");
        assert_eq!(label, "Enterprise");
        assert!(cap.is_none(), "enterprise has no cap");
    }

    #[test]
    fn plan_tier_info_hides_unknown_tiers() {
        assert!(plan_tier_info("none").is_none());
        assert!(plan_tier_info("").is_none());
        assert!(plan_tier_info("nonsense").is_none());
    }

    #[test]
    fn plan_tier_info_is_case_insensitive() {
        assert!(plan_tier_info("PRO").is_some());
        assert!(plan_tier_info("Max20").is_some());
        assert!(plan_tier_info(" team ").is_some());
    }

    #[test]
    fn keys_all_none_by_default() {
        let k = KeysConfig::default();
        assert!(k.bookmark.is_none());
        assert!(k.export.is_none());
        assert!(k.delete.is_none());
        assert!(k.rename.is_none());
        assert!(k.copy_id.is_none());
        assert!(k.copy_path.is_none());
        assert!(k.open_editor.is_none());
    }
}
