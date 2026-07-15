# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Fast, harness-independent terminal statusline: renders a color-coded status
  block from harness stdin JSON (Claude Code, Cursor CLI) plus live git state.
- Adaptive input parser that pulls model, cost, context, and rate-limit fields
  from any of several known harness JSON shapes — no per-harness configuration.
- Git state via a single `git status --porcelain=v2` subprocess, with an extra
  `git diff --numstat` only when the tree has tracked changes, and an optional
  `git describe` when `[git] tags = true`.
- Session cost, active-API burn rate (idle-excluded), and 5-hour / 7-day
  rate-limit windows with reset countdowns.
- TOML configuration (`./.statusline.toml`, `~/.claude/statusline.toml`, or
  `~/.config/statusline/config.toml`) with env var overrides (`NO_COLOR`,
  `FORCE_COLOR`, `STATUSLINE_LAYOUT`, `STATUSLINE_GLYPHS`).
- Glyph auto-detection (Nerd Font / Unicode / ASCII) and terminal
  color-capability detection (truecolor / 256 / 16 / none).
- Per-segment toggles (`[segments]`) and single-line or multi-row layout.
