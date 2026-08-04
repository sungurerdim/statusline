//! TOML configuration with layered precedence.
//!
//! Load order (first found wins, no merging — keep it predictable):
//!   1. `./.statusline.toml`                (project)
//!   2. `$CLAUDE_CONFIG_DIR/statusline.toml` or `~/.claude/statusline.toml`
//!   3. `$XDG_CONFIG_HOME/statusline/config.toml` or `~/.config/statusline/config.toml`
//!
//! Missing/invalid file → built-in defaults (the tool must always render).
//!
//! Every option is a typed enum rather than a free string, so a typo
//! (`color = "trucolor"`) is reported on stderr instead of silently degrading
//! to whatever the fallback arm happened to be.
//!
//! Env vars (`STATUSLINE_LAYOUT`, `NO_COLOR`, …) still apply and take
//! precedence over the file for the values they cover.

use crate::git::{GitOptions, UntrackedMode};
use crate::model::BurnMode;
use crate::render::Layout;
use crate::theme::{ColorMode, GlyphMode};
use serde::Deserialize;
use std::env;
use std::path::PathBuf;

/// Row layout. `multi` is the default aligned block.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LayoutOpt {
    Multi,
    Single,
}

/// Icon vocabulary. `auto` defers to locale/terminal detection.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GlyphOpt {
    Auto,
    Ascii,
    Unicode,
    Nerd,
}

/// Color depth. `auto` defers to `NO_COLOR`/`COLORTERM`/`TERM` detection.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ColorOpt {
    Auto,
    Off,
    #[serde(rename = "16")]
    Ansi16,
    #[serde(rename = "256")]
    Ansi256,
    Truecolor,
}

/// Burn-rate denominator.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BurnOpt {
    Api,
    Wall,
    Off,
}

/// How much of the untracked set `git status` should scan.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UntrackedOpt {
    No,
    Normal,
    All,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub layout: Option<LayoutOpt>,
    pub glyphs: Option<GlyphOpt>,
    pub color: Option<ColorOpt>,
    pub burn: Option<BurnOpt>,
    pub git: GitConfig,
    pub segments: Segments,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GitConfig {
    pub untracked: UntrackedOpt,
    pub tags: bool,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            untracked: UntrackedOpt::Normal,
            tags: false,
        }
    }
}

/// Per-segment on/off. All default to on.
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Segments {
    pub git: bool,
    pub model: bool,
    pub context: bool,
    pub lines: bool,
    pub changes: bool,
    pub cost: bool,
    pub block: bool,
    pub week: bool,
}

impl Default for Segments {
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

impl Config {
    /// Load from the first existing config path, or defaults.
    pub fn load() -> Self {
        for path in candidate_paths() {
            if let Ok(text) = std::fs::read_to_string(&path) {
                // A malformed config should not break the statusline: warn to
                // stderr (harmless — statusline reads stdout) and use defaults.
                match toml::from_str::<Config>(&text) {
                    Ok(cfg) => return cfg,
                    Err(e) => {
                        eprintln!("statusline: ignoring invalid config {path:?}: {e}");
                        return Config::default();
                    }
                }
            }
        }
        Config::default()
    }

    pub fn git_options(&self) -> GitOptions {
        let untracked = match self.git.untracked {
            UntrackedOpt::No => UntrackedMode::No,
            UntrackedOpt::All => UntrackedMode::All,
            UntrackedOpt::Normal => UntrackedMode::Normal,
        };
        GitOptions {
            untracked,
            tags: self.git.tags,
            lines: self.segments.lines,
        }
    }

    /// Resolve layout: env `STATUSLINE_LAYOUT` overrides file overrides default.
    pub fn layout(&self) -> Layout {
        if let Ok(v) = env::var("STATUSLINE_LAYOUT") {
            return match v.as_str() {
                "single" => Layout::Single,
                _ => Layout::Multi,
            };
        }
        match self.layout {
            Some(LayoutOpt::Single) => Layout::Single,
            _ => Layout::Multi,
        }
    }

    /// Resolve glyph mode: file `auto`/absent → env/locale detection.
    pub fn glyph_mode(&self) -> GlyphMode {
        match self.glyphs {
            Some(GlyphOpt::Ascii) => GlyphMode::Ascii,
            Some(GlyphOpt::Unicode) => GlyphMode::Unicode,
            Some(GlyphOpt::Nerd) => GlyphMode::Nerd,
            Some(GlyphOpt::Auto) | None => crate::theme::detect_glyph_mode(),
        }
    }

    /// Resolve burn-rate basis (default: API/active time).
    pub fn burn_mode(&self) -> BurnMode {
        match self.burn {
            Some(BurnOpt::Wall) => BurnMode::Wall,
            Some(BurnOpt::Off) => BurnMode::Off,
            Some(BurnOpt::Api) | None => BurnMode::Api,
        }
    }

    /// Resolve color mode: file explicit value wins, else env detection. `off`
    /// is always honored via detection too (NO_COLOR), so detection stays last.
    pub fn color_mode(&self) -> ColorMode {
        match self.color {
            Some(ColorOpt::Off) => ColorMode::None,
            Some(ColorOpt::Ansi16) => ColorMode::Ansi16,
            Some(ColorOpt::Ansi256) => ColorMode::Ansi256,
            Some(ColorOpt::Truecolor) => ColorMode::Truecolor,
            Some(ColorOpt::Auto) | None => crate::theme::detect_color_mode(),
        }
    }
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    paths.push(PathBuf::from("./.statusline.toml"));

    if let Ok(dir) = env::var("CLAUDE_CONFIG_DIR") {
        paths.push(PathBuf::from(dir).join("statusline.toml"));
    } else if let Some(home) = home_dir() {
        paths.push(home.join(".claude").join("statusline.toml"));
    }

    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        paths.push(PathBuf::from(xdg).join("statusline").join("config.toml"));
    } else if let Some(home) = home_dir() {
        paths.push(home.join(".config").join("statusline").join("config.toml"));
    }
    paths
}

fn home_dir() -> Option<PathBuf> {
    env::var("HOME").ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_all_on() {
        let c = Config::default();
        assert!(c.segments.git && c.segments.cost && c.segments.block);
        assert_eq!(c.git.untracked, UntrackedOpt::Normal);
        assert!(!c.git.tags);
    }

    #[test]
    fn parses_partial_toml() {
        let c: Config = toml::from_str(
            r#"
            layout = "single"
            glyphs = "nerd"
            [git]
            untracked = "no"
            tags = true
            [segments]
            block = false
        "#,
        )
        .unwrap();
        assert_eq!(c.layout, Some(LayoutOpt::Single));
        assert_eq!(c.glyph_mode(), GlyphMode::Nerd);
        assert_eq!(c.git_options().untracked, UntrackedMode::No);
        assert!(c.git.tags);
        assert!(!c.segments.block);
        assert!(c.segments.git); // untouched → default on
    }

    #[test]
    fn unknown_key_is_rejected() {
        let r: Result<Config, _> = toml::from_str("bogus_key = 1");
        assert!(r.is_err());
    }

    /// BP-004 regression: before typed options, a misspelled *value* fell
    /// through the catch-all arm and silently produced the default. A typo in
    /// a config must surface, not quietly change behaviour.
    #[test]
    fn invalid_value_is_rejected() {
        for bad in [
            r#"color = "trucolor""#,
            r#"layout = "sngle""#,
            r#"glyphs = "nerdfont""#,
            r#"burn = "wallclock""#,
            "[git]\nuntracked = \"banana\"",
        ] {
            let r: Result<Config, _> = toml::from_str(bad);
            assert!(r.is_err(), "expected {bad:?} to be rejected");
        }
    }

    #[test]
    fn color_override_wins() {
        let c: Config = toml::from_str(r#"color = "off""#).unwrap();
        assert_eq!(c.color_mode(), ColorMode::None);
    }

    #[test]
    fn numeric_color_levels_parse() {
        let c: Config = toml::from_str(r#"color = "256""#).unwrap();
        assert_eq!(c.color_mode(), ColorMode::Ansi256);
        let c: Config = toml::from_str(r#"color = "16""#).unwrap();
        assert_eq!(c.color_mode(), ColorMode::Ansi16);
    }

    #[test]
    fn burn_mode_resolves() {
        assert_eq!(Config::default().burn_mode(), BurnMode::Api);
        let w: Config = toml::from_str(r#"burn = "wall""#).unwrap();
        assert_eq!(w.burn_mode(), BurnMode::Wall);
        let o: Config = toml::from_str(r#"burn = "off""#).unwrap();
        assert_eq!(o.burn_mode(), BurnMode::Off);
    }

    /// `lines = false` must reach the git layer so the diff spawn is skipped.
    #[test]
    fn lines_segment_flows_into_git_options() {
        let c: Config = toml::from_str("[segments]\nlines = false").unwrap();
        assert!(!c.git_options().lines);
        assert!(Config::default().git_options().lines);
    }
}
