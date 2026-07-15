//! TOML configuration with layered precedence.
//!
//! Load order (first found wins, no merging — keep it predictable):
//!   1. `./.statusline.toml`                (project)
//!   2. `$CLAUDE_CONFIG_DIR/statusline.toml` or `~/.claude/statusline.toml`
//!   3. `$XDG_CONFIG_HOME/statusline/config.toml` or `~/.config/statusline/config.toml`
//!
//! Missing/invalid file → built-in defaults (the tool must always render).
//!
//! Everything here is optional; env vars (`STATUSLINE_LAYOUT`, `NO_COLOR`, …)
//! still apply and take precedence over the file for the values they cover.

use crate::git::{GitOptions, UntrackedMode};
use crate::model::BurnMode;
use crate::render::Layout;
use crate::theme::{ColorMode, GlyphMode};
use serde::Deserialize;
use std::env;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// "multi" (default) or "single".
    pub layout: Option<String>,
    /// "auto" (default), "ascii", "unicode", "nerd".
    pub glyphs: Option<String>,
    /// "auto" (default), "off", "16", "256", "truecolor".
    pub color: Option<String>,
    /// Burn-rate basis: "api" (default, idle-excluded), "wall", "off".
    pub burn: Option<String>,
    pub git: GitConfig,
    pub segments: Segments,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GitConfig {
    /// "normal" (default), "no", "all".
    pub untracked: String,
    pub tags: bool,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            untracked: "normal".into(),
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
        let untracked = match self.git.untracked.as_str() {
            "no" => UntrackedMode::No,
            "all" => UntrackedMode::All,
            _ => UntrackedMode::Normal,
        };
        GitOptions {
            untracked,
            tags: self.git.tags,
        }
    }

    /// Resolve layout: env `STATUSLINE_LAYOUT` overrides file overrides default.
    pub fn layout(&self) -> Layout {
        let v = env::var("STATUSLINE_LAYOUT")
            .ok()
            .or_else(|| self.layout.clone());
        match v.as_deref() {
            Some("single") => Layout::Single,
            _ => Layout::Multi,
        }
    }

    /// Resolve glyph mode: file "auto"/absent → env/locale detection.
    pub fn glyph_mode(&self) -> GlyphMode {
        match self.glyphs.as_deref() {
            Some("ascii") => GlyphMode::Ascii,
            Some("unicode") => GlyphMode::Unicode,
            Some("nerd") => GlyphMode::Nerd,
            _ => crate::theme::detect_glyph_mode(),
        }
    }

    /// Resolve burn-rate basis (default: API/active time).
    pub fn burn_mode(&self) -> BurnMode {
        match self.burn.as_deref() {
            Some("wall") => BurnMode::Wall,
            Some("off") => BurnMode::Off,
            _ => BurnMode::Api,
        }
    }

    /// Resolve color mode: file explicit value wins, else env detection. `off`
    /// is always honored via detection too (NO_COLOR), so detection stays last.
    pub fn color_mode(&self) -> ColorMode {
        match self.color.as_deref() {
            Some("off") => ColorMode::None,
            Some("16") => ColorMode::Ansi16,
            Some("256") => ColorMode::Ansi256,
            Some("truecolor") => ColorMode::Truecolor,
            _ => crate::theme::detect_color_mode(),
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
        assert_eq!(c.git.untracked, "normal");
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
        assert_eq!(c.layout.as_deref(), Some("single"));
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

    #[test]
    fn color_override_wins() {
        let c: Config = toml::from_str(r#"color = "off""#).unwrap();
        assert_eq!(c.color_mode(), ColorMode::None);
    }

    #[test]
    fn burn_mode_resolves() {
        assert_eq!(Config::default().burn_mode(), BurnMode::Api);
        let w: Config = toml::from_str(r#"burn = "wall""#).unwrap();
        assert_eq!(w.burn_mode(), BurnMode::Wall);
        let o: Config = toml::from_str(r#"burn = "off""#).unwrap();
        assert_eq!(o.burn_mode(), BurnMode::Off);
    }
}
