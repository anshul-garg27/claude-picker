//! Persisted monthly budget for the `--stats` dashboard.
//!
//! The dashboard shows three budget things at the bottom of the screen:
//!
//! 1. **Forecast** — a projection of end-of-month spend, computed from
//!    month-to-date / day-of-month (a simple linear extrapolation). Always
//!    visible, color-coded by magnitude so users catch runaway spend.
//! 2. **Month-to-date** — sum of costs across sessions whose last timestamp
//!    falls in the current calendar month.
//! 3. **Progress toward user-set limit** — only when the user has run the
//!    budget modal (`b` keybinding) and set `monthly_limit_usd > 0`.
//!
//! The modal persists its value to `~/.config/claude-picker/budget.toml`
//! so it survives across runs without the user having to export an env
//! var or edit the main `config.toml`. A missing file is NOT an error —
//! `load()` falls back to a default (no limit set).
//!
//! File shape:
//!
//! ```toml
//! monthly_limit_usd = 300.0
//! # optional — show forecast at all? Users who find it anxiety-inducing
//! # can disable via the `f` key, which sets this to false.
//! show_forecast = true
//! ```
//!
//! This is the one piece of stats-dashboard state that lives outside of
//! `StatsData` — it's a user preference, not a computation.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Persisted budget configuration.
///
/// All fields default to sensible "no-limit" values, so a missing or empty
/// file deserialises cleanly without blocking the dashboard from rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    /// Monthly spend cap in USD. `0.0` (the default) means "no limit set —
    /// don't render the progress bar". Users set this via the `b` modal.
    #[serde(default)]
    pub monthly_limit_usd: f64,

    /// Whether the forecast band is visible. Toggled by the `f` key — some
    /// users prefer not to see a scary projection every day. Default `true`
    /// so the feature is discoverable on first run.
    #[serde(default = "default_show_forecast")]
    pub show_forecast: bool,
}

fn default_show_forecast() -> bool {
    true
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            monthly_limit_usd: 0.0,
            show_forecast: true,
        }
    }
}

impl Budget {
    /// Default on-disk path: `~/.config/claude-picker/budget.toml`. Returns
    /// `None` on headless hosts where we can't find a home directory.
    pub fn default_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".config").join("claude-picker").join("budget.toml"))
    }

    /// Load from the default path, falling back to [`Budget::default`] if
    /// the file is missing or malformed. We deliberately swallow parse
    /// errors here (unlike `Config::load_from` in the main config module)
    /// because the budget file is set via the in-TUI modal, not hand-edited;
    /// a corruption should never stop the dashboard from rendering — we
    /// just forget the limit.
    pub fn load() -> Self {
        match Self::default_path() {
            Some(path) => Self::load_from(&path).unwrap_or_default(),
            None => Self::default(),
        }
    }

    /// Injection-friendly variant. Returns an error only for truly
    /// unrecoverable I/O (permission denied, etc.); a missing file is
    /// reported as `Ok(Self::default())`.
    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path)
            .with_context(|| format!("reading budget at {}", path.display()))?;
        let parsed: Self = toml::from_str(&raw)
            .with_context(|| format!("parsing budget TOML at {}", path.display()))?;
        Ok(parsed)
    }

    /// Persist to the default path, creating the parent directory if it
    /// doesn't exist. This is what the budget modal calls when the user
    /// hits Enter to confirm a new limit.
    pub fn save(&self) -> Result<()> {
        let path = Self::default_path()
            .ok_or_else(|| anyhow::anyhow!("no home directory to persist budget"))?;
        self.save_to(&path)
    }

    /// Injection-friendly variant of [`save`](Self::save).
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating {} parent dir", path.display()))?;
        }
        let serialized = toml::to_string_pretty(self).context("serializing budget to TOML")?;
        fs::write(path, serialized).with_context(|| format!("writing to {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn defaults_have_no_limit_and_show_forecast() {
        let b = Budget::default();
        assert!((b.monthly_limit_usd - 0.0).abs() < f64::EPSILON);
        assert!(b.show_forecast, "show_forecast default should be true");
    }

    #[test]
    fn missing_file_returns_defaults() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("nope.toml");
        let b = Budget::load_from(&path).expect("missing is ok");
        assert!(b.monthly_limit_usd.abs() < f64::EPSILON);
    }

    #[test]
    fn roundtrip_save_and_load() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("budget.toml");
        let b = Budget {
            monthly_limit_usd: 300.0,
            show_forecast: false,
        };
        b.save_to(&path).expect("save");
        let loaded = Budget::load_from(&path).expect("load");
        assert!((loaded.monthly_limit_usd - 300.0).abs() < 1e-9);
        assert!(!loaded.show_forecast);
    }

    #[test]
    fn malformed_toml_errors_clearly() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("budget.toml");
        fs::write(&path, "not = valid = toml").expect("write");
        let err = Budget::load_from(&path).expect_err("must fail");
        assert!(format!("{err:#}").contains("parsing budget TOML"));
    }

    #[test]
    fn save_creates_parent_dir() {
        let tmp = tempdir().expect("tempdir");
        let nested = tmp.path().join("a").join("b").join("budget.toml");
        let b = Budget {
            monthly_limit_usd: 50.0,
            show_forecast: true,
        };
        b.save_to(&nested).expect("save creates dir");
        assert!(nested.is_file());
    }

    #[test]
    fn partial_file_keeps_defaults_for_missing_fields() {
        // Only the limit is set; show_forecast must fall through to the
        // `default_show_forecast` = true.
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("budget.toml");
        fs::write(&path, "monthly_limit_usd = 100.0\n").expect("write");
        let b = Budget::load_from(&path).expect("parse");
        assert!((b.monthly_limit_usd - 100.0).abs() < 1e-9);
        assert!(b.show_forecast);
    }
}
