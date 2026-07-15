//! Harness-agnostic normalized status data.
//!
//! Every input adapter (Claude Code JSON, generic env/args, ...) produces a
//! [`StatusData`]. Every segment renders from it. This is the single seam that
//! makes the tool harness-independent: adapters differ, this struct does not.

/// Git working-tree summary (filled by the git engine, `None` outside a repo).
// Scaffold: fields/helpers consumed by the render layer in Phase 3 (segments).
#[allow(dead_code)]
#[derive(Debug, Default, Clone)]
pub struct GitInfo {
    pub repo_name: Option<String>,
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// staged counts
    pub staged_mod: u32,
    pub staged_add: u32,
    pub staged_del: u32,
    pub staged_ren: u32,
    /// working-tree (unstaged) counts
    pub wt_mod: u32,
    pub wt_del: u32,
    pub untracked: u32,
    pub conflicts: u32,
    pub stashes: u32,
    pub detached: bool,
    pub tag: Option<String>,
    /// Uncommitted line changes vs HEAD (git diff --numstat). Reset on commit.
    pub diff_added: u32,
    pub diff_removed: u32,
}

#[allow(dead_code)] // totals consumed by the git segment in Phase 3
impl GitInfo {
    /// Total modified across staged + unstaged.
    pub fn total_mod(&self) -> u32 {
        self.staged_mod + self.wt_mod
    }
    pub fn total_add(&self) -> u32 {
        self.staged_add + self.untracked
    }
    pub fn total_del(&self) -> u32 {
        self.staged_del + self.wt_del
    }
    pub fn total_ren(&self) -> u32 {
        self.staged_ren
    }

    /// True when there are tracked (staged or unstaged) changes — the only case
    /// where a `git diff` line count is meaningful. Untracked-only stays false
    /// so we skip the extra diff spawn on an otherwise-clean tree.
    pub fn has_tracked_changes(&self) -> bool {
        self.staged_mod
            + self.staged_add
            + self.staged_del
            + self.staged_ren
            + self.wt_mod
            + self.wt_del
            > 0
    }
}

/// A rate-limit window (Claude Code `rate_limits.five_hour` / `seven_day`).
// Scaffold: read by the rate-limit segment in Phase 3.
#[allow(dead_code)]
#[derive(Debug, Default, Clone)]
pub struct RateWindow {
    pub used_pct: f64,
    /// unix epoch seconds when the window resets
    pub resets_at: Option<i64>,
}

/// Normalized, harness-agnostic status data consumed by all segments.
// Scaffold: display fields are read by segments in Phase 3; `cwd`/`project_name`
// /`git` are already used by the input+git pipeline.
#[allow(dead_code)]
#[derive(Debug, Default, Clone)]
pub struct StatusData {
    // location
    pub cwd: Option<String>,
    pub project_name: Option<String>,

    // model / harness
    pub model_name: Option<String>,
    pub harness_version: Option<String>,
    pub effort_level: Option<String>,
    pub output_style: Option<String>,

    // context window (input-token based, matching Claude Code semantics)
    pub context_tokens: Option<u64>,
    pub context_size: Option<u64>,
    pub context_used_pct: Option<f64>,

    // cost / effort
    pub cost_usd: Option<f64>,
    pub duration_ms: Option<u64>,
    /// Active API/compute time — excludes idle/thinking gaps, so it yields a
    /// far more realistic burn rate than wall-clock duration.
    pub api_duration_ms: Option<u64>,

    // rate-limit windows
    pub five_hour: Option<RateWindow>,
    pub seven_day: Option<RateWindow>,

    // git (filled later by the git engine)
    pub git: Option<GitInfo>,
}

/// How to compute the burn-rate denominator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BurnMode {
    /// Active API/compute time (idle-excluded) — the default; falls back to
    /// wall-clock when API time isn't provided.
    Api,
    /// Wall-clock session duration (includes idle/thinking gaps).
    Wall,
    /// Don't show a burn rate at all.
    Off,
}

impl StatusData {
    /// Burn rate in USD per hour, per [`BurnMode`]. `None` when off, data is
    /// missing, or the window is too small to be meaningful.
    pub fn burn_usd_per_hr(&self, mode: BurnMode) -> Option<f64> {
        let cost = self.cost_usd?;
        let ms = match mode {
            BurnMode::Off => return None,
            BurnMode::Wall => self.duration_ms?,
            BurnMode::Api => self
                .api_duration_ms
                .filter(|&m| m > 0)
                .or(self.duration_ms)?,
        };
        if ms < 1000 {
            return None; // < 1s: not enough signal, avoid divide-by-noise
        }
        Some(cost / (ms as f64 / 3_600_000.0))
    }
}
