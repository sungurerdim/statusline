//! Input layer: turn any harness's stdin payload into normalized [`StatusData`].
//!
//! A single [`adaptive`] parser handles every harness by pulling known concepts
//! from multiple candidate field paths — Claude Code, Cursor CLI, OpenCode's
//! proposed shape, Codex's, or a bare generic object all work with no config.

pub mod adaptive;

use crate::model::StatusData;

/// Parse whatever arrived on stdin. Blank input yields an empty [`StatusData`]
/// so the tool still renders (e.g. a git-only line in generic/shell mode).
pub fn from_stdin(raw: &str) -> StatusData {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return StatusData::default();
    }
    adaptive::parse(trimmed)
}
