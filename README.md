# statusline

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A fast, harness-independent terminal statusline — one small binary that renders a
rich, color-coded status block from whatever your AI coding harness (Claude Code,
Cursor CLI, …) sends on stdin, plus live git state.

No runtime, no Node, no `ccusage` subprocess. Pure Rust, one static binary.

```
⌥ slcmp:main △1  ·  ◉ Opus 4.8 high  ·  ◔ 214K 22%
+1 −0 · ~1 +1 −0 · $15.26 🔥20.63/h · 5h 68% ↻2h5m · 7d 41% ↻6d22h
```

Row 1 — repo:branch + ahead/behind (or ✓ when fully synced), model + reasoning
effort, context window %.
Row 2 — uncommitted line diff, working-tree file changes, session cost + burn
rate, and the 5-hour / 7-day rate-limit windows with reset countdowns.

## Why

Built to replace an older Go statusline and to fix what the ecosystem's tools miss:

- **Everything from stdin — no cost guessing.** Cost, context %, rate limits and
  reasoning effort come straight from the harness's own accounting (Claude Code's
  `total_cost_usd` is provider-priced with cache tokens). Tools that re-estimate
  cost from a bundled price table (ccusage, OpenCode) drift; this doesn't.
- **Realistic burn rate.** `$/hr` is computed over *active API time*, not wall
  clock, so idle and thinking gaps don't dilute it (configurable).
- **Git that reflects reality.** The `+/−` line diff is the live uncommitted diff
  vs `HEAD` — it clears the moment you commit. A green `✓` shows when the tree is
  clean and fully in sync with the remote.
- **Adaptive input.** A single parser pulls each field from any of several known
  JSON shapes (Claude Code, Cursor CLI, and other harnesses' proposed schemas),
  using whatever is present and omitting what isn't — no per-harness build.
- **Cheap.** One `git status` (with `--no-optional-locks`, so it never writes a
  lock), a second `git diff` only when the tree is dirty, no temp files, no disk
  writes. ~460 KB binary, ~8 ms per render.

## Old vs new

Same input, old Go `cco-statusline` vs this:

```
OLD  slcmp:main
     △ 1 ▽ 0 · mod 1 · add 1 · del 0 · mv 0
     sungur · CC 2.1.210 · Opus 4.8 · 214K 21%

NEW  ⌥ slcmp:main △1  ·  ◉ Opus 4.8 high  ·  ◔ 214K 22%
     +1 −0 · ~1 +1 −0 · $15.26 🔥20.63/h · 5h 68% ↻2h5m · 7d 41% ↻6d22h
```

| | Old (Go) | New (Rust) |
|---|---|---|
| Binary size | 2.0 MB | **460 KB** (~4.4× smaller) |
| CPU / 100 renders | 2.01 s (186% — many git spawns) | **0.68 s** (~2.9× less) |
| Wall / render | ~10.8 ms | **~8.1 ms** |
| git subprocesses | 3–4 per render | **1** (+1 diff only when dirty) |
| Session cost $ | ✗ | ✓ |
| Burn rate ($/hr, idle-excluded) | ✗ | ✓ |
| 5h / 7d rate limits + reset countdown | ✗ | ✓ |
| Uncommitted line diff (clears on commit) | ✗ (showed file counts only) | ✓ |
| "Synced with remote" ✓ marker | ✗ | ✓ |
| Reasoning effort | ✗ | ✓ |
| Configurable (layout/glyphs/color/burn/segments) | ✗ | ✓ (TOML) |
| Harness-adaptive input | ✗ (fixed schema) | ✓ |
| Glyph auto-detect (Nerd/Unicode/ASCII) + `NO_COLOR` | partial | ✓ |

## Install

```bash
cargo build --release
cp target/release/statusline ~/.claude/statusline
```

Point your harness at it. For **Claude Code**, in `~/.claude/settings.json`:

```json
"statusLine": {
  "type": "command",
  "command": "~/.claude/statusline",
  "padding": 1,
  "refreshInterval": 3
}
```

`refreshInterval` (seconds) keeps time-based bits (reset countdowns) and git
state fresh while the session is idle — e.g. right after you commit or push.

**Cursor CLI** uses the same Claude-Code-compatible stdin JSON, so the same
command works. In a plain shell / tmux with no harness JSON, it renders a
git-only line.

## Configuration

Optional TOML, first found wins:

1. `./.statusline.toml` (per project)
2. `~/.claude/statusline.toml` (or `$CLAUDE_CONFIG_DIR/statusline.toml`)
3. `~/.config/statusline/config.toml` (or `$XDG_CONFIG_HOME/...`)

Every option is documented in [`statusline.example.toml`](statusline.example.toml).
Highlights: `layout = "multi" | "single"`, `glyphs = "auto" | "nerd" | "unicode" |
"ascii"`, `color`, `burn = "api" | "wall" | "off"`, per-`[segments]` toggles, and
`[git]` untracked/tag options. Env vars (`NO_COLOR`, `FORCE_COLOR`,
`STATUSLINE_LAYOUT`, `STATUSLINE_GLYPHS`) override the file.

## Harness support

| Harness | Status |
|---|---|
| Claude Code | Full — all fields |
| Cursor CLI | Same stdin schema — whatever fields it provides |
| Generic shell / tmux | git-only (no session cost/context outside a harness) |
| Codex / Gemini CLI / Crush / OpenCode | Not yet — they don't run command-backed statuslines (fixed-enum or none). The adaptive parser is ready for their proposed schemas once shipped. |

Cost/context/rate-limit data only exists inside a harness that produces it; this
tool faithfully shows whatever it receives and omits the rest — it never prints
made-up values.

## License

MIT
