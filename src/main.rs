//! statusline — fast, harness-independent terminal statusline.
//!
//! Reads harness input (Claude Code / Cursor CLI JSON) on stdin, normalizes it,
//! collects git state, and renders a configurable status block to stdout.

mod config;
mod git;
mod input;
mod model;
mod render;
mod theme;

use config::Config;
use render::{RenderCtx, SegmentFlags};
use std::io::Read;
use theme::Glyphs;

fn main() {
    let mut raw = String::new();
    // Non-fatal: if stdin can't be read we proceed with empty input.
    let _ = std::io::stdin().read_to_string(&mut raw);

    let cfg = Config::load();
    let mut data = input::from_stdin(&raw);

    // Generic/shell mode: no harness JSON → anchor on the real working dir so
    // git runs in the right place and the repo name is meaningful.
    if data.cwd.is_none()
        && let Ok(dir) = std::env::current_dir()
    {
        if data.project_name.is_none() {
            data.project_name = dir.file_name().and_then(|n| n.to_str()).map(str::to_string);
        }
        data.cwd = dir.to_str().map(str::to_string);
    }

    data.git = git::collect(
        data.cwd.as_deref(),
        cfg.git_options(),
        data.project_name.as_deref(),
    );

    let ctx = RenderCtx {
        color: cfg.color_mode(),
        glyphs: Glyphs::for_mode(cfg.glyph_mode()),
        layout: cfg.layout(),
        segments: SegmentFlags {
            git: cfg.segments.git,
            model: cfg.segments.model,
            context: cfg.segments.context,
            lines: cfg.segments.lines,
            changes: cfg.segments.changes,
            cost: cfg.segments.cost,
            block: cfg.segments.block,
            week: cfg.segments.week,
        },
        burn: cfg.burn_mode(),
    };

    println!("{}", render::render(&data, &ctx));
}
