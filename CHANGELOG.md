# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2026-08-04

### Fixed

- Nothing a harness sends can stretch the bar any more. Boundary fuzzing found
  that every rendered field was unbounded: a 5000-character model name produced
  a 5013-character status line, `resets_at = i64::MAX` rendered as
  `106751991146631d1h`, and `used_percentage: 1e9` printed `1000000000%`. Model
  names, effort levels, repo/branch names and tags are now elided with a visible
  `...`, percentages clamp at `999+%`, and countdowns at `99d+`. Nothing is
  invented — over-long values are cut and the cut is marked.
- A negative or non-finite cost is no longer rendered. `$-5.00` with a matching
  negative burn rate is not a value a session can produce; the segment is now
  omitted, which is what the tool already does for data it does not have.
- The release workflow no longer swallows every `gh release create` failure.
  Only "the release already exists" is tolerated; auth and permission errors
  fail the job instead of reporting success with nothing published.

### Changed

- `Glyphs::all()` destructures exhaustively, so adding a glyph fails to compile
  until it is added to the width invariant — previously the guard could silently
  stop covering new glyphs.
- The benchmark moved from `scripts/bench.sh` to `tests/bench.rs`
  (`cargo test --release --test bench -- --ignored --nocapture`), so it runs on
  Windows too rather than requiring a POSIX shell.
- The coverage job builds `cargo-llvm-cov` from source instead of downloading a
  third-party prebuilt binary, removing that surface from the pipeline.
- Test fixtures use pid-qualified temp directories, so concurrent test runs on
  one machine cannot race on the same path.
- README performance figures re-measured with the portable benchmark; the
  unreproducible CPU-per-100-renders row was dropped rather than left standing.

## [0.2.0] - 2026-08-04

### Fixed

- Stalled `git` subprocesses are terminated on every supported platform again.
  v0.1.1 replaced the cross-platform `Child::kill()` with a Unix-only `kill`
  spawn, so on Windows a hung child was never stopped. The timeout path now
  uses `kill` on Unix and `taskkill /T /F` on Windows.
- `git diff --numstat` line counts saturate at `u32::MAX` instead of wrapping
  in release builds or panicking in debug builds on a pathological diff.
- A failed waiter-thread spawn now signals the child instead of abandoning it.

### Changed

- Config values are typed rather than free strings: an unknown value
  (`color = "trucolor"`, `[git] untracked = "banana"`) is now reported on
  stderr and falls back to defaults, where it previously slipped through a
  catch-all arm and silently changed behaviour.
- `visible_len` moved from `render` to `theme`, beside the glyph table it has
  to agree with. A new invariant test fails if a glyph is added whose width the
  measurement code does not recognise — previously that silently misaligned
  every row containing it.
- Unused palette constants (`BLUE`, `TEAL`, `TEXT`) and the
  `#[allow(dead_code)]` hiding them were removed.
- Dependencies: serde 1.0.229, serde_json 1.0.151, toml 1.1.4; pinned action
  SHAs refreshed.

### Added

- Declared MSRV (`rust-version = "1.85"`), enforced by a CI job that builds on
  exactly that toolchain.
- CI runs the full suite on macOS, Linux, and Windows, all with `--locked` so a
  green run reflects the committed `Cargo.lock`. Coverage is measured via
  `cargo llvm-cov`.
- Integration tests (`tests/cli.rs`) covering the real binary end to end:
  full harness payload, bare stdin, malformed stdin, and running outside a repo.
- Unit tests for `burn_usd_per_hr` (api/wall/off, sub-second guard, zero-API
  fallback), previously untested.
- `scripts/bench.sh` — the reproducible benchmark the README's numbers come from.
- Release workflow publishing binaries for macOS (arm64/x86_64), Linux, and
  Windows on tag push.
- `.github/CODEOWNERS`, and a platform-support section in the README.

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
