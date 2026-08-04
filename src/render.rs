//! Segment building + layout.
//!
//! Turns a [`StatusData`] into the final multi-line (default) or single-line
//! string. Segments that have no data are omitted rather than shown empty.

use crate::model::{BurnMode, RateWindow, StatusData};
use crate::theme::{Color, ColorMode, Glyphs, Palette, paint, visible_len};
use std::time::{SystemTime, UNIX_EPOCH};

/// Which segments to render (mirrors `[segments]` config; all on by default).
#[derive(Debug, Clone, Copy)]
pub struct SegmentFlags {
    pub git: bool,
    pub model: bool,
    pub context: bool,
    pub lines: bool,
    pub changes: bool,
    pub cost: bool,
    pub block: bool,
    pub week: bool,
}

impl Default for SegmentFlags {
    fn default() -> Self {
        Self {
            git: true,
            model: true,
            context: true,
            lines: true,
            changes: true,
            cost: true,
            block: true,
            week: true,
        }
    }
}

/// Rendering context: resolved capabilities + layout choice.
pub struct RenderCtx {
    pub color: ColorMode,
    pub glyphs: Glyphs,
    pub layout: Layout,
    pub segments: SegmentFlags,
    pub burn: BurnMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Aligned multi-row block (default).
    Multi,
    /// Single dense line.
    Single,
}

impl RenderCtx {
    fn p(&self, text: &str, color: Color, bold: bool) -> String {
        paint(text, color, self.color, bold)
    }
}

// ---- input bounds ------------------------------------------------------------
//
// The bar is a fixed-width widget, but every number and string in it arrives
// from a harness we do not control. Rendering a field at whatever size it
// arrives pushes the line past the terminal and destroys the alignment of every
// row — a 5000-character model name produced a 5013-character status line.
//
// Eliding keeps the "never print a value we made up" rule intact: nothing is
// invented, an over-long value is cut and the cut is marked.

/// Model names beyond this are elided. Comfortably fits every real model id.
const MAX_MODEL_CHARS: usize = 40;
/// Percentages above this are implausible; show the ceiling, marked.
const MAX_PCT: f64 = 999.0;
/// Reset countdowns beyond this are implausible; show the ceiling, marked.
const MAX_DURATION_DAYS: i64 = 99;

/// Cut `s` to `max` characters, marking the cut. ASCII marker so it is safe in
/// every glyph mode, including non-UTF-8 terminals.
fn elide(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let kept: String = s.chars().take(max.saturating_sub(3)).collect();
    format!("{kept}...")
}

/// Render a percentage, clamped to a plausible range. Values outside it are
/// shown at the bound with a `+`/`-` marker rather than verbatim.
fn fmt_pct(pct: f64) -> String {
    if !pct.is_finite() {
        return "?%".to_string();
    }
    if pct > MAX_PCT {
        return format!("{}+%", MAX_PCT as i64);
    }
    if pct < 0.0 {
        return "0%".to_string();
    }
    format!("{}%", pct.round() as i64)
}

/// Format a token count compactly: 170000 -> "170K", 1500000 -> "1.5M".
fn fmt_k(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{}K", (n as f64 / 1000.0).round() as u64)
    } else {
        n.to_string()
    }
}

/// Prefix `text` with `glyph` and a space, or just `text` when glyph is empty
/// (ASCII mode leaves some glyphs blank — avoids a stray leading space).
fn with_glyph(glyph: &str, text: &str) -> String {
    if glyph.is_empty() {
        text.to_string()
    } else {
        format!("{glyph} {text}")
    }
}

/// Seconds remaining until `resets_at` (unix epoch), or `None` if it is now/past
/// or the clock is unavailable.
fn remaining_secs(resets_at: i64) -> Option<i64> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    let rem = resets_at - now;
    (rem > 0).then_some(rem)
}

/// Compact duration: `2d3h`, `4h12m`, `9m` (never zero-padded, coarsest 2 units).
fn fmt_duration(secs: i64) -> String {
    let d = secs / 86_400;
    if d > MAX_DURATION_DAYS {
        return format!("{MAX_DURATION_DAYS}d+");
    }
    let h = (secs % 86_400) / 3600;
    let m = (secs % 3600) / 60;
    if d > 0 {
        format!("{d}d{h}h")
    } else if h > 0 {
        format!("{h}h{m}m")
    } else {
        format!("{}m", m.max(1))
    }
}

/// Render a rate-limit window: `<glyph> N%` colored by threshold, plus a dim
/// reset countdown when a future `resets_at` is known.
fn rate_segment(glyph: &str, w: &RateWindow, ctx: &RenderCtx) -> String {
    let mut s = ctx.p(
        &format!("{glyph} {}", fmt_pct(w.used_pct)),
        pct_color(w.used_pct),
        false,
    );
    if let Some(rem) = w.resets_at.and_then(remaining_secs) {
        s.push_str(&ctx.p(
            &format!(" {}{}", ctx.glyphs.reset, fmt_duration(rem)),
            Palette::GRAY,
            false,
        ));
    }
    s
}

/// Color for a percent-used value: calm below 70, warn to 90, danger above.
fn pct_color(pct: f64) -> Color {
    if pct >= 90.0 {
        Palette::RED
    } else if pct >= 70.0 {
        Palette::YELLOW
    } else {
        Palette::SKY
    }
}

// ---- segment builders: each returns None when it has nothing to show --------

fn seg_git(d: &StatusData, ctx: &RenderCtx) -> Option<String> {
    let Some(git) = &d.git else {
        return Some(ctx.p("no git", Palette::GRAY, false));
    };
    let branch = git
        .branch
        .as_deref()
        .unwrap_or(if git.detached { "detached" } else { "?" });
    let repo = git
        .repo_name
        .as_deref()
        .or(d.project_name.as_deref())
        .unwrap_or("repo");
    let mut s = ctx.p(
        &with_glyph(
            ctx.glyphs.branch,
            &elide(&format!("{repo}:{branch}"), MAX_MODEL_CHARS),
        ),
        Palette::GREEN,
        true,
    );

    if git.ahead > 0 {
        s.push(' ');
        s.push_str(&ctx.p(
            &format!("{}{}", ctx.glyphs.ahead, git.ahead),
            Palette::GREEN,
            false,
        ));
    }
    if git.behind > 0 {
        s.push(' ');
        s.push_str(&ctx.p(
            &format!("{}{}", ctx.glyphs.behind, git.behind),
            Palette::YELLOW,
            false,
        ));
    }
    if git.conflicts > 0 {
        s.push(' ');
        s.push_str(&ctx.p(
            &format!("{}{}", ctx.glyphs.conflict, git.conflicts),
            Palette::RED,
            true,
        ));
    }
    if git.stashes > 0 {
        s.push(' ');
        s.push_str(&ctx.p(
            &format!("{}{}", ctx.glyphs.stash, git.stashes),
            Palette::GRAY,
            false,
        ));
    }
    if let Some(tag) = &git.tag {
        s.push(' ');
        s.push_str(&ctx.p(
            &format!("{}{}", ctx.glyphs.tag, elide(tag, MAX_MODEL_CHARS)),
            Palette::GRAY,
            false,
        ));
    }
    // Fully in sync with the remote: clean tree, nothing to push/pull, upstream
    // set. A positive "all good" marker, mirroring the commit/push-needed cues.
    let clean = !git.has_tracked_changes() && git.untracked == 0 && git.conflicts == 0;
    let synced = git.upstream.is_some() && git.ahead == 0 && git.behind == 0;
    if clean && synced {
        s.push(' ');
        s.push_str(&ctx.p(ctx.glyphs.synced, Palette::GREEN, false));
    }
    Some(s)
}

fn seg_model(d: &StatusData, ctx: &RenderCtx) -> Option<String> {
    let name = elide(d.model_name.as_deref()?, MAX_MODEL_CHARS);
    let mut s = ctx.p(&with_glyph(ctx.glyphs.model, &name), Palette::MAUVE, false);
    if let Some(level) = d.effort_level.as_deref()
        && level != "medium"
        && !level.is_empty()
    {
        s.push_str(&ctx.p(
            &format!(" {}", elide(level, MAX_MODEL_CHARS)),
            Palette::GRAY,
            false,
        ));
    }
    Some(s)
}

fn seg_context(d: &StatusData, ctx: &RenderCtx) -> Option<String> {
    let pct = d.context_used_pct?;
    let tokens = d.context_tokens.map(fmt_k).unwrap_or_default();
    let body = if tokens.is_empty() {
        format!("{} {}", ctx.glyphs.context, fmt_pct(pct))
    } else {
        format!("{} {tokens} {}", ctx.glyphs.context, fmt_pct(pct))
    };
    Some(ctx.p(&body, pct_color(pct), false))
}

fn seg_lines(d: &StatusData, ctx: &RenderCtx) -> Option<String> {
    // Uncommitted line changes vs HEAD — reflects the live git working tree, so
    // it clears after a commit (unlike a session-cumulative edit count).
    let git = d.git.as_ref()?;
    let (added, removed) = (git.diff_added, git.diff_removed);
    if added == 0 && removed == 0 {
        return None;
    }
    let mut s = ctx.p(
        &format!("{}{added}", ctx.glyphs.added),
        Palette::GREEN,
        false,
    );
    s.push(' ');
    s.push_str(&ctx.p(
        &format!("{}{removed}", ctx.glyphs.removed),
        Palette::RED,
        false,
    ));
    Some(s)
}

fn seg_changes(d: &StatusData, ctx: &RenderCtx) -> Option<String> {
    let git = d.git.as_ref()?;
    let (m, a, del) = (git.total_mod(), git.total_add(), git.total_del());
    if m == 0 && a == 0 && del == 0 {
        return None;
    }
    let part = |n: u32, glyph: &str, color: Color| -> String {
        let c = if n > 0 { color } else { Palette::GRAY };
        ctx.p(&format!("{glyph}{n}"), c, false)
    };
    Some(format!(
        "{} {} {}",
        part(m, ctx.glyphs.modified, Palette::YELLOW),
        part(a, ctx.glyphs.added, Palette::GREEN),
        part(del, ctx.glyphs.removed, Palette::RED),
    ))
}

fn seg_cost(d: &StatusData, ctx: &RenderCtx) -> Option<String> {
    let cost = d.cost_usd?;
    // A negative or non-finite cost is not a value the session could have
    // produced. Showing nothing is honest; showing "$-5.00" is not.
    if !cost.is_finite() || cost < 0.0 {
        return None;
    }
    let mut s = ctx.p(
        &format!("{}{cost:.2}", ctx.glyphs.cost),
        Palette::YELLOW,
        false,
    );
    if let Some(burn) = d.burn_usd_per_hr(ctx.burn) {
        s.push(' ');
        s.push_str(&ctx.p(
            &format!("{}{burn:.2}/h", ctx.glyphs.burn),
            Palette::PEACH,
            false,
        ));
    }
    Some(s)
}

fn seg_block(d: &StatusData, ctx: &RenderCtx) -> Option<String> {
    Some(rate_segment(ctx.glyphs.block, d.five_hour.as_ref()?, ctx))
}

fn seg_week(d: &StatusData, ctx: &RenderCtx) -> Option<String> {
    Some(rate_segment(ctx.glyphs.week, d.seven_day.as_ref()?, ctx))
}

// ---- layout -----------------------------------------------------------------

/// Spread `parts` across `target` width, distributing padding around a colored
/// separator so multiple rows line up into an aligned block.
fn justify(parts: &[String], target: usize, sep: &str, ctx: &RenderCtx) -> String {
    if parts.is_empty() {
        return String::new();
    }
    if parts.len() == 1 {
        return parts[0].clone();
    }
    let gaps = parts.len() - 1;
    let sep_w = visible_len(sep);
    let content: usize = parts.iter().map(|p| visible_len(p)).sum();
    let avail = target.saturating_sub(content + gaps * sep_w);
    let per = avail / gaps;
    let extra = avail % gaps;

    let colored_sep = ctx.p(sep, Palette::GRAY, false);
    let mut out = parts[0].clone();
    for (i, part) in parts.iter().enumerate().skip(1) {
        let gap = per + if i <= extra { 1 } else { 0 };
        let left = gap / 2;
        let right = gap - left;
        out.push_str(&" ".repeat(left));
        out.push_str(&colored_sep);
        out.push_str(&" ".repeat(right));
        out.push_str(part);
    }
    out
}

fn min_width(parts: &[String], sep: &str) -> usize {
    if parts.is_empty() {
        return 0;
    }
    let sep_w = visible_len(sep) + 2; // " sep "
    parts.iter().map(|p| visible_len(p)).sum::<usize>() + (parts.len() - 1) * sep_w
}

/// Render the final status string.
pub fn render(d: &StatusData, ctx: &RenderCtx) -> String {
    let s = &ctx.segments;
    let gate = |on: bool, v: Option<String>| if on { v } else { None };

    let row1: Vec<String> = [
        gate(s.git, seg_git(d, ctx)),
        gate(s.model, seg_model(d, ctx)),
        gate(s.context, seg_context(d, ctx)),
    ]
    .into_iter()
    .flatten()
    .collect();
    let row2: Vec<String> = [
        gate(s.lines, seg_lines(d, ctx)),
        gate(s.changes, seg_changes(d, ctx)),
        gate(s.cost, seg_cost(d, ctx)),
        gate(s.block, seg_block(d, ctx)),
        gate(s.week, seg_week(d, ctx)),
    ]
    .into_iter()
    .flatten()
    .collect();

    let sep = ctx.glyphs.sep;

    match ctx.layout {
        Layout::Single => {
            let mut all: Vec<String> = row1;
            all.extend(row2);
            let colored_sep = ctx.p(sep, Palette::GRAY, false);
            all.join(&format!(" {colored_sep} "))
        }
        Layout::Multi => {
            let rows: Vec<&Vec<String>> = [&row1, &row2]
                .into_iter()
                .filter(|r| !r.is_empty())
                .collect();
            let target = rows.iter().map(|r| min_width(r, sep)).max().unwrap_or(0);
            let lines: Vec<String> = rows.iter().map(|r| justify(r, target, sep, ctx)).collect();
            // trailing zero-width line keeps Claude Code from trimming the block
            format!("{}\n\u{200B}", lines.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::GitInfo;

    fn ascii_ctx(layout: Layout) -> RenderCtx {
        RenderCtx {
            color: ColorMode::None,
            glyphs: Glyphs::for_mode(crate::theme::GlyphMode::Ascii),
            layout,
            segments: SegmentFlags::default(),
            burn: BurnMode::Api,
        }
    }

    /// BP-101 regression: a harness field must never be able to set the width
    /// of the bar. Fuzzing found a 5000-char model name producing a 5013-char
    /// line, which destroys the alignment of every row.
    #[test]
    fn an_absurd_model_name_cannot_stretch_the_line() {
        let d = StatusData {
            model_name: Some("A".repeat(5000)),
            ..Default::default()
        };
        let out = render(&d, &ascii_ctx(Layout::Single));
        assert!(
            out.chars().count() < 120,
            "line grew to {} chars: {out}",
            out.chars().count()
        );
        assert!(out.contains("..."), "the cut must be visible: {out}");
    }

    #[test]
    fn elide_keeps_short_values_untouched() {
        assert_eq!(elide("Opus 5", 40), "Opus 5");
        assert_eq!(elide("abcdef", 6), "abcdef");
        assert_eq!(elide("abcdefg", 6), "abc...");
    }

    /// BP-104 regression: `resets_at = i64::MAX` rendered as
    /// `↻106751991146631d1h`.
    #[test]
    fn fmt_duration_has_a_ceiling() {
        assert_eq!(fmt_duration(3600), "1h0m");
        assert_eq!(fmt_duration(99 * 86_400), "99d0h");
        assert_eq!(fmt_duration(i64::MAX), "99d+");
    }

    /// BP-105 regression: `used_percentage: 1e9` printed `1000000000%`.
    #[test]
    fn fmt_pct_clamps_implausible_values() {
        assert_eq!(fmt_pct(17.4), "17%");
        assert_eq!(fmt_pct(1e9), "999+%");
        assert_eq!(fmt_pct(-5.0), "0%");
        assert_eq!(fmt_pct(f64::NAN), "?%");
        assert_eq!(fmt_pct(f64::INFINITY), "?%");
    }

    /// BP-106 regression: a negative cost rendered as `$-5.00` with a matching
    /// negative burn rate. Nothing is a truthful rendering of a value the
    /// session cannot have produced.
    #[test]
    fn implausible_cost_is_omitted_rather_than_shown() {
        for bad in [-5.0, f64::NAN, f64::INFINITY] {
            let d = StatusData {
                cost_usd: Some(bad),
                duration_ms: Some(3_600_000),
                ..Default::default()
            };
            let out = render(&d, &ascii_ctx(Layout::Single));
            assert!(!out.contains('$'), "cost {bad} should be hidden: {out}");
        }
    }

    #[test]
    fn a_very_long_branch_name_is_elided_too() {
        let d = StatusData {
            git: Some(GitInfo {
                repo_name: Some("repo".into()),
                branch: Some("feature/".to_string() + &"x".repeat(500)),
                ..Default::default()
            }),
            ..Default::default()
        };
        let out = render(&d, &ascii_ctx(Layout::Single));
        assert!(out.chars().count() < 120, "{out}");
    }

    #[test]
    fn fmt_k_scales() {
        assert_eq!(fmt_k(500), "500");
        assert_eq!(fmt_k(170_000), "170K");
        assert_eq!(fmt_k(1_500_000), "1.5M");
    }

    #[test]
    fn renders_no_color_ascii_block() {
        let mut d = StatusData {
            model_name: Some("Opus 4.8".into()),
            context_tokens: Some(170_000),
            context_used_pct: Some(17.0),
            cost_usd: Some(0.42),
            duration_ms: Some(4_980_000),
            ..Default::default()
        };
        d.git = Some(GitInfo {
            repo_name: Some("statusline".into()),
            branch: Some("main".into()),
            ahead: 2,
            wt_mod: 3,
            untracked: 1,
            diff_added: 156,
            diff_removed: 80,
            ..Default::default()
        });
        let out = render(&d, &ascii_ctx(Layout::Multi));
        assert!(out.contains("statusline:main"), "{out}");
        assert!(out.contains("Opus 4.8"), "{out}");
        assert!(out.contains("17%"), "{out}");
        assert!(out.contains("$0.42"), "{out}");
        assert!(out.contains("^2")); // ahead
        assert!(out.contains("+156") && out.contains("-80")); // uncommitted diff
        // no ANSI escapes in None mode
        assert!(!out.contains('\x1b'));
    }

    #[test]
    fn single_line_joins_with_sep() {
        let d = StatusData {
            model_name: Some("Opus".into()),
            ..Default::default()
        };
        let out = render(&d, &ascii_ctx(Layout::Single));
        assert!(out.contains('|'), "{out}");
        assert!(!out.contains('\n'));
    }

    #[test]
    fn duration_formats_coarsely() {
        assert_eq!(fmt_duration(90 * 60), "1h30m");
        assert_eq!(fmt_duration(2 * 86_400 + 3 * 3600), "2d3h");
        assert_eq!(fmt_duration(45 * 60), "45m");
        assert_eq!(fmt_duration(30), "1m"); // never "0m"
    }

    #[test]
    fn synced_marker_only_when_clean_and_synced() {
        let mut d = StatusData {
            git: Some(GitInfo {
                branch: Some("main".into()),
                repo_name: Some("r".into()),
                upstream: Some("origin/main".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let out = render(&d, &ascii_ctx(Layout::Multi));
        assert!(out.contains("r:main ok"), "synced: {out}");

        // A dirty tree must NOT show the synced marker.
        d.git.as_mut().unwrap().wt_mod = 1;
        let out = render(&d, &ascii_ctx(Layout::Multi));
        assert!(!out.contains("ok"), "not synced: {out}");
    }

    #[test]
    fn rate_segment_appends_countdown() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let w = RateWindow {
            used_pct: 50.0,
            resets_at: Some(now + 90 * 60), // 1h30m ahead
        };
        let ctx = ascii_ctx(Layout::Multi);
        let out = rate_segment(ctx.glyphs.block, &w, &ctx);
        assert!(out.contains("5h 50%"), "{out}");
        assert!(
            out.contains("1h29m") || out.contains("1h30m"),
            "countdown: {out}"
        );
    }

    #[test]
    fn tag_renders_when_present() {
        let d = StatusData {
            git: Some(GitInfo {
                branch: Some("main".into()),
                repo_name: Some("statusline".into()),
                tag: Some("v0.1.0".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let out = render(&d, &ascii_ctx(Layout::Multi));
        assert!(out.contains("@v0.1.0"), "{out}");
    }

    /// Render-coverage guard (BP-010): every populated `StatusData`/`GitInfo`
    /// field must surface somewhere in `render()` output. Catches the class of
    /// bug where a field is parsed/computed but never wired into a segment
    /// (see ds/audit/findings.md BP-001/BP-003/BP-004 history).
    #[test]
    fn every_populated_field_surfaces_in_output() {
        let d = StatusData {
            model_name: Some("Opus 4.8".into()),
            effort_level: Some("high".into()),
            context_tokens: Some(170_000),
            context_used_pct: Some(17.0),
            cost_usd: Some(0.42),
            duration_ms: Some(4_980_000),
            api_duration_ms: Some(2_000_000),
            five_hour: Some(RateWindow {
                used_pct: 68.0,
                resets_at: None,
            }),
            seven_day: Some(RateWindow {
                used_pct: 41.0,
                resets_at: None,
            }),
            git: Some(GitInfo {
                repo_name: Some("statusline".into()),
                branch: Some("main".into()),
                upstream: Some("origin/main".into()),
                ahead: 1,
                staged_mod: 1,
                staged_add: 1,
                wt_mod: 1,
                untracked: 1,
                conflicts: 1,
                stashes: 1,
                tag: Some("v0.1.0".into()),
                diff_added: 1,
                diff_removed: 1,
                ..Default::default()
            }),
            ..Default::default()
        };
        let out = render(&d, &ascii_ctx(Layout::Multi));
        for expected in [
            "main", "Opus 4.8", "high", "17%", "$0.42", "/h", "^1", "!1", "*1", "@v0.1.0", "+1",
            "-1", "~2", "5h 68%", "7d 41%",
        ] {
            assert!(out.contains(expected), "missing {expected:?} in {out}");
        }
    }
}
