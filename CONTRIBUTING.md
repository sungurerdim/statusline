# Contributing

Thanks for considering a contribution. This is a small, single-binary Rust
project — the bar for a PR is: it builds, it's tested, and it fits the
existing style.

## Local setup

```bash
git clone https://github.com/sungurerdim/statusline.git
cd statusline
cargo build --release
```

Requires a recent stable Rust toolchain (edition 2024, `rustc` 1.85+).

## Before opening a PR

Run the full local quality gate — all four must be clean:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
```

## Testing

- Every bug fix needs a regression test.
- New segments/config options need at least one test covering the happy path
  and one boundary/edge case.
- Tests that mutate process env vars (see `src/theme.rs`) must serialize on
  the existing `ENV_LOCK` mutex and restore whatever they changed — env vars
  are process-global, so unguarded mutation flakes other tests.
- Keep test data realistic (real-shaped model names, costs, durations, git
  output) — match the existing fixtures in `src/input/adaptive.rs` and
  `src/git.rs`.

## Style

- Match the existing module boundaries: `config` (TOML/env resolution),
  `git` (subprocess + parsing), `input` (harness JSON → `StatusData`),
  `model` (harness-agnostic data), `render` (segments + layout), `theme`
  (color/glyphs).
- `cargo fmt` output is final — don't hand-format around it.
- Comments explain *why*, not *what* — the code already says what.

## Pull requests

- Keep PRs focused on one change.
- Describe the behavior change, not the implementation, in the PR description.
- CI (once set up) must pass before merge.
