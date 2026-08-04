# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-08-04

### Fixed

- Git commands producing more than the ~64 KiB pipe buffer (e.g.
  `status -uall` in a large tree) no longer deadlock. The subprocess waiter
  polled `try_wait()` without ever reading the child's pipes, so the child
  blocked on `write` and was killed at the 2s timeout — git state was silently
  dropped on exactly the repos where it matters most.

### Changed

- Subprocess waiting blocks on a worker thread instead of polling on a 5ms
  sleep, removing the latency quantum that rounded every git call up to the
  next 5ms boundary. Measured on a dirty 10-file repo: p50 15.9ms → 13.6ms,
  worst case 21.7ms → 15.1ms, with byte-identical output.
- `[segments] lines = false` now also skips the `git diff --numstat`
  subprocess, which previously ran even when its output was never rendered.
  Measured on the same repo: p50 15.9ms → 8.4ms.

## [0.1.0] - 2026-07-29

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
