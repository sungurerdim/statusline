//! Theme: semantic color palette + glyph set, with automatic capability
//! detection and config override.
//!
//! Color palette is a Catppuccin-Mocha-inspired set — a popular, high-contrast,
//! accessible dark palette — mapped to semantic roles (branch, model, cost, …).
//! Colors degrade truecolor → 256 → 16 → none based on the terminal.
//!
//! Glyphs are auto-managed: universal Unicode by default (renders in almost any
//! modern font), upgraded to Nerd-Font icons only when opted in, downgraded to
//! pure ASCII on non-UTF-8 / dumb terminals. All overridable via config/env.

use std::env;

// ============================================================================
// Color
// ============================================================================

/// A semantic color: truecolor RGB plus a 16-color ANSI fallback code (30-37 /
/// 90-97) used when the terminal can't do 256/truecolor.
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// SGR code for the 16-color fallback (e.g. 92 = bright green).
    pub ansi16: u8,
}

impl Color {
    const fn new(r: u8, g: u8, b: u8, ansi16: u8) -> Self {
        Self { r, g, b, ansi16 }
    }
}

/// How much color the terminal supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    None,
    Ansi16,
    Ansi256,
    Truecolor,
}

/// Semantic palette. One entry per meaning, not per literal color, so the whole
/// look can be retuned in one place.
pub struct Palette;

// A curated, complete semantic token set. Some tones (info/user/text) are
// reserved for config-selectable segments not enabled by default.
#[allow(dead_code)]
impl Palette {
    // Catppuccin Mocha hexes with sensible bright-ANSI fallbacks.
    pub const GREEN: Color = Color::new(0xa6, 0xe3, 0xa1, 92); // branch, additions
    pub const MAUVE: Color = Color::new(0xcb, 0xa6, 0xf7, 95); // model
    pub const BLUE: Color = Color::new(0x89, 0xb4, 0xfa, 94); // info
    pub const SKY: Color = Color::new(0x89, 0xdc, 0xeb, 96); // context, accent
    pub const YELLOW: Color = Color::new(0xf9, 0xe2, 0xaf, 93); // modified, cost
    pub const PEACH: Color = Color::new(0xfa, 0xb3, 0x87, 91); // burn, warning
    pub const RED: Color = Color::new(0xf3, 0x8b, 0xa8, 91); // danger, deletions
    pub const TEAL: Color = Color::new(0x94, 0xe2, 0xd5, 96); // user
    pub const GRAY: Color = Color::new(0x6c, 0x70, 0x86, 90); // dim, separators
    pub const TEXT: Color = Color::new(0xcd, 0xd6, 0xf4, 97); // default text
}

/// Wrap `text` in SGR codes for `color`, honoring the color mode. Bold optional.
pub fn paint(text: &str, color: Color, mode: ColorMode, bold: bool) -> String {
    if mode == ColorMode::None {
        return text.to_string();
    }
    let b = if bold { "1;" } else { "" };
    let fg = match mode {
        ColorMode::Truecolor => format!("38;2;{};{};{}", color.r, color.g, color.b),
        ColorMode::Ansi256 => format!("38;5;{}", rgb_to_256(color.r, color.g, color.b)),
        ColorMode::Ansi16 => color.ansi16.to_string(),
        ColorMode::None => unreachable!(),
    };
    format!("\x1b[{b}{fg}m{text}\x1b[0m")
}

/// Convert 24-bit RGB to the nearest xterm-256 index (6×6×6 cube + grayscale).
fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    // Grayscale ramp if the channel spread is tight.
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    if max - min < 10 {
        // 24-step gray ramp lives at 232..=255
        if max < 8 {
            return 16;
        }
        if max > 248 {
            return 231;
        }
        return 232 + ((max as u16 - 8) * 24 / 247) as u8;
    }
    let c = |v: u8| -> u16 {
        if v < 48 {
            0
        } else if v < 115 {
            1
        } else {
            ((v as u16 - 35) / 40).min(5)
        }
    };
    (16 + 36 * c(r) + 6 * c(g) + c(b)) as u8
}

/// Detect terminal color support from env (`NO_COLOR`, `FORCE_COLOR`,
/// `COLORTERM`, `TERM`). Config can override this at a higher layer.
pub fn detect_color_mode() -> ColorMode {
    // FORCE_COLOR wins both ways (0 = off, else on).
    if let Ok(fc) = env::var("FORCE_COLOR") {
        if fc == "0" || fc.eq_ignore_ascii_case("false") {
            return ColorMode::None;
        }
        return best_color_level();
    }
    // NO_COLOR: any non-empty value disables color (per no-color.org).
    if env::var("NO_COLOR").map(|v| !v.is_empty()).unwrap_or(false) {
        return ColorMode::None;
    }
    if env::var("TERM").map(|t| t == "dumb").unwrap_or(false) {
        return ColorMode::None;
    }
    best_color_level()
}

fn best_color_level() -> ColorMode {
    if let Ok(ct) = env::var("COLORTERM")
        && (ct.contains("truecolor") || ct.contains("24bit"))
    {
        return ColorMode::Truecolor;
    }
    match env::var("TERM") {
        Ok(t) if t.contains("256") => ColorMode::Ansi256,
        Ok(t) if t.is_empty() => ColorMode::Ansi16,
        Ok(_) => ColorMode::Ansi16,
        Err(_) => ColorMode::Ansi256, // no TERM (e.g. piped by a harness): assume modern
    }
}

// ============================================================================
// Glyphs
// ============================================================================

/// Which icon vocabulary to render with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphMode {
    /// Pure ASCII — safe on any terminal/font.
    Ascii,
    /// Universal Unicode symbols present in almost all modern fonts.
    Unicode,
    /// Nerd-Font Private-Use-Area icons (opt-in; needs a patched font).
    Nerd,
}

/// The concrete glyphs a segment may use, resolved for the active mode.
pub struct Glyphs {
    pub branch: &'static str,
    pub ahead: &'static str,
    pub behind: &'static str,
    pub synced: &'static str,
    pub model: &'static str,
    pub context: &'static str,
    pub cost: &'static str,
    pub burn: &'static str,
    pub block: &'static str,
    pub week: &'static str,
    pub reset: &'static str,
    pub added: &'static str,
    pub removed: &'static str,
    pub modified: &'static str,
    pub conflict: &'static str,
    pub stash: &'static str,
    pub sep: &'static str,
}

impl Glyphs {
    pub fn for_mode(mode: GlyphMode) -> Self {
        match mode {
            GlyphMode::Ascii => Glyphs {
                branch: "",
                ahead: "^",
                behind: "v",
                synced: "ok",
                model: "",
                context: "ctx",
                cost: "$",
                burn: "~",
                block: "5h",
                week: "7d",
                reset: "~",
                added: "+",
                removed: "-",
                modified: "~",
                conflict: "!",
                stash: "*",
                sep: "|",
            },
            GlyphMode::Unicode => Glyphs {
                branch: "\u{2325}",  // ⌥ (option) as a light branch mark
                ahead: "\u{25b3}",   // △
                behind: "\u{25bd}",  // ▽
                synced: "\u{2713}",  // ✓
                model: "\u{25c9}",   // ◉
                context: "\u{25d4}", // ◔
                cost: "$",
                burn: "\u{1f525}", // 🔥 (widely supported emoji)
                block: "5h",
                week: "7d",
                reset: "\u{21bb}", // ↻
                added: "+",
                removed: "\u{2212}", // −
                modified: "~",
                conflict: "\u{26a0}", // ⚠
                stash: "\u{2691}",    // ⚑
                sep: "\u{00b7}",      // ·
            },
            GlyphMode::Nerd => Glyphs {
                branch: "\u{e0a0}",   // powerline branch
                ahead: "\u{f062}",    // arrow up
                behind: "\u{f063}",   // arrow down
                synced: "\u{f00c}",   // check
                model: "\u{f2db}",    // chip
                context: "\u{f1fe}",  // gauge
                cost: "\u{f155}",     // dollar
                burn: "\u{f490}",     // flame-ish
                block: "\u{f017}",    // clock
                week: "\u{f073}",     // calendar
                reset: "\u{f021}",    // refresh
                added: "\u{f067}",    // plus
                removed: "\u{f068}",  // minus
                modified: "\u{f040}", // pencil
                conflict: "\u{f421}", // git-merge conflict
                stash: "\u{f01c}",    // inbox
                sep: "\u{00b7}",      // ·
            },
        }
    }
}

/// Detect the glyph mode from locale/terminal + `STATUSLINE_GLYPHS` env.
/// Config overrides this at a higher layer.
pub fn detect_glyph_mode() -> GlyphMode {
    if let Ok(v) = env::var("STATUSLINE_GLYPHS") {
        match v.to_ascii_lowercase().as_str() {
            "nerd" => return GlyphMode::Nerd,
            "ascii" => return GlyphMode::Ascii,
            "unicode" => return GlyphMode::Unicode,
            _ => {}
        }
    }
    if env::var("TERM").map(|t| t == "dumb").unwrap_or(false) || !locale_is_utf8() {
        return GlyphMode::Ascii;
    }
    // Universal Unicode is the safe, font-agnostic default.
    GlyphMode::Unicode
}

fn locale_is_utf8() -> bool {
    for key in ["LC_ALL", "LC_CTYPE", "LANG"] {
        if let Ok(v) = env::var(key)
            && !v.is_empty()
        {
            return v.to_ascii_lowercase().contains("utf-8")
                || v.to_ascii_lowercase().contains("utf8");
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_mode_strips_color() {
        assert_eq!(paint("x", Palette::GREEN, ColorMode::None, false), "x");
    }

    #[test]
    fn truecolor_emits_rgb() {
        let s = paint("x", Palette::GREEN, ColorMode::Truecolor, false);
        assert!(s.contains("38;2;166;227;161"), "{s}");
        assert!(s.ends_with("\x1b[0m"));
    }

    #[test]
    fn ansi16_uses_fallback_code() {
        let s = paint("x", Palette::GREEN, ColorMode::Ansi16, true);
        assert!(s.contains("1;92m"), "{s}");
    }

    #[test]
    fn rgb_to_256_known_points() {
        assert_eq!(rgb_to_256(0, 0, 0), 16); // black
        assert_eq!(rgb_to_256(255, 255, 255), 231); // white cube corner
        // mid gray lands in the gray ramp
        let g = rgb_to_256(128, 128, 128);
        assert!((232..=255).contains(&g), "gray idx {g}");
    }

    #[test]
    fn glyphs_ascii_are_plain() {
        let g = Glyphs::for_mode(GlyphMode::Ascii);
        assert_eq!(g.ahead, "^");
        assert_eq!(g.sep, "|");
        assert!(g.branch.is_ascii());
    }
}
