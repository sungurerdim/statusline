# Statusline / Prompt Tool Competitive Landscape — Research Artifact

**Generated:** 2026-07-15 · **Scope:** Claude Code statusline ecosystem, the Claude Code statusLine stdin mechanism, statusline support across other AI coding harnesses, and design/perf lessons from general terminal prompt tools (starship, oh-my-posh, powerlevel10k, tmux, spaceship) plus git-status performance techniques (gitstatusd).

**Purpose:** inform a ground-up rebuild of a fast, harness-independent statusline binary.

**Confidence:** MEDIUM-HIGH. Most data points confirmed from ≥2 independent sources (official docs + repo README + community write-ups). A few numbers (perf benchmarks, download counts, version numbers) are single-source and are flagged `[single-source]` — these are self-reported by tool authors and not independently re-benchmarked by this research pass.

---

## 1. Per-tool comparison table — Claude Code statusline ecosystem

| Tool | Lang / Runtime | Install | Data shown | Config format | Theming / glyphs | Layout | Notable |
|---|---|---|---|---|---|---|---|
| **ccstatusline** ([sirmalloc/ccstatusline](https://github.com/sirmalloc/ccstatusline)) | TypeScript 99.2% (React + Ink for TUI), runs on Node.js or Bun | `npx ccstatusline@latest` / `bunx ccstatusline@latest` / `npm i -g ccstatusline` | model, git branch, token usage, per-model weekly usage (Sonnet/Opus split), extra usage limits, voice-input state, session duration, compaction count, 5h block timer, git insertions/deletions, cache hit-rate/read/write, context bar | JSON at `~/.config/ccstatusline/settings.json`; `--config <path>` override; `CLAUDE_CONFIG_DIR` env respected | Powerline arrows + custom caps, basic/256/truecolor incl. hex, multi-stop gradients, auto Nerd-Font install w/ consent | Multi-line (multiple independent status lines configurable) | Widest feature surface of the JS-based tools; interactive TUI config editor; npm provenance attestations added for supply-chain trust; largest community adoption signal (3.1k★ per search-engine snapshot, single-source) |
| **claude-powerline** ([Owloops/claude-powerline](https://github.com/Owloops/claude-powerline)) | Node.js 18+ (JS/TS), requires Git 2.0+ | `npx -y @owloops/claude-powerline@latest --style=powerline` wired into `settings.json`; also installable as a plugin | vim-style powerline segments: model, directory, git (branch, SHA, tags, stash count, ahead/behind), session cost/budget tracking, response time, lines added/removed, 5h block utilization (bar/blocks/dots/etc. styles) | JSON at `./.claude-powerline.json` (project), `~/.claude/claude-powerline.json` (user), or `~/.config/claude-powerline/config.json` (XDG); precedence CLI flag > env > config file > default | 6 built-in themes (dark, light, nord, tokyo-night, rose-pine, gruvbox); Nerd Font recommended, `--charset=text` ASCII fallback; visual web configurator at powerline.owloops.com | Both classic single-line powerline **and** a CSS-Grid-inspired multi-row "TUI" panel mode with responsive breakpoints | Only tool offering a live web-based visual configurator; explicit rate-limit block-window integration requiring Claude Code's native `rate_limits` hook data |
| **cc-statusline** ([chongdashu/cc-statusline](https://github.com/chongdashu/cc-statusline)) | TypeScript (68.4%) CLI generator that emits a Shell script (31.6%) | `npm i -g @chongdashu/cc-statusline`; generates `.claude/statusline.sh` and wires `settings.json` | model, ccusage-sourced live usage stats, colors forced for Claude Code terminal (respects `NO_COLOR`) | Generated bash script (not a live config file); interactive CLI wizard at generation time | ANSI colors only (no powerline glyphs documented) | Single-line (generated script) | Self-reports perf: execution <100ms target / 45-80ms typical, <5MB memory, jq as only dependency; ships a file-based locking mechanism around ccusage calls specifically to prevent concurrent-process pile-up (a documented failure mode of naive ccusage integrations) `[single-source: repo README benchmark table]` |
| **ccusage statusline** ([ryoppippi/ccusage](https://github.com/ryoppippi/ccusage), [ccusage.com/guide/statusline](https://ccusage.com/guide/statusline)) | TypeScript, run via `bunx`/`npx`/`claude x` (native bun-bundled binary) | Add to `settings.json`: `{"statusLine":{"type":"command","command":"BUN_BE_BUN=1 claude x ccusage statusline"}}` | current session cost, today's total cost, current 5h session-block cost + time remaining, burn rate ($/hr, color-coded by tokens/min thresholds), active model + reasoning effort, context tokens+% (color thresholds configurable via `--context-low-threshold` etc.) | CLI flags + config file; model-label aliasing supported | Emoji-based (🤖💰🔥🧠), not powerline; color thresholds configurable | Single-line | Labeled **Beta**; documented, reproduced bug: statusline invocation can cause infinite process-spawning / 100%+ CPU loops (GitHub issues #455, #459) because each invocation can spawn a heavy multi-hundred-MB process and, if it doesn't return fast enough, Claude Code re-invokes it — a cautionary data point for a from-scratch binary's exec model |
| **ccusage-statusline-rs** ([ticpu/ccusage-statusline-rs](https://github.com/ticpu/ccusage-statusline-rs)) | Rust | cargo/binary | Reimplements ccusage's statusline output & pricing logic | n/a (mirrors ccusage) | n/a | Single-line | Self-reported **15x faster than the Node.js implementation (8ms vs 120ms warm)** `[single-source]` — direct evidence that the ccusage Node implementation is the slow baseline being targeted by Rust rewrites |
| **CCometixLine** ([Haleclipse/CCometixLine](https://github.com/Haleclipse/CCometixLine)) | Rust | `npm install -g @cometix/ccline` (npm-distributed Rust binary) | Directory, Git, Model, Usage, Time, Cost, OutputStyle segments — each independently toggleable with custom separators/icons/colors | TOML-like interactive TUI config (segment toggles) | Custom separators + icons per segment | Single-line | Positioned explicitly as "high-performance … written in Rust"; ships companion "Claude Code enhancement utilities" beyond just the statusline |
| **claude-statusline (Go)** ([TheoBrigitte/claude-statusline](https://github.com/TheoBrigitte/claude-statusline)) | Go | `go install` | n/a (not deep-dived this pass) | n/a | n/a | n/a | Self-reported: full pipeline (config load, JSON decode, render, write) completes in **~19µs with 78 allocs**; render alone ~5µs `[single-source]` — the fastest self-reported number found in this research; indicates a compiled, allocation-conscious binary can be ~1000x faster than a cold Node.js process |
| **claude-code-statusline (sotayamashita)** | Rust | n/a | starship-like segment config | starship-inspired config | n/a | n/a | Ships its own criterion benchmark suite with a documented threshold check of **<50ms mean** — notable as the only tool in this set treating perf regression testing as a first-class CI concern `[single-source]` |
| **CCstatus (MaurUppi)** | Rust | n/a | Adds network-probing to diagnose relay/latency issues (distinct feature vs. peers) | n/a | n/a | n/a | Niche differentiator: active network latency diagnostics baked into a statusline tool |

**Cross-cutting pattern confirmed by ≥2 independent sources (search-engine roundups + individual repo READMEs):** the ecosystem is bifurcating into (a) feature-rich, npm/Node-based, actively-themed tools (ccstatusline, claude-powerline) that accept slower cold-start in exchange for TUI configurators and rich widget catalogs, and (b) a growing wave of Rust/Go rewrites (ccusage-statusline-rs, CCometixLine, claude-statusline, claude-code-statusline, CCstatus) whose primary value proposition is raw speed against a Node.js cold-start baseline that multiple sources measure at 45-120ms+ vs. single-digit-ms or microsecond compiled alternatives.

---

## 2. Claude Code `statusLine` mechanism — confirmed stdin JSON schema

**Source:** [code.claude.com/docs/en/statusline](https://code.claude.com/docs/en/statusline) (official Anthropic docs), cross-checked against community field-reference guides (AKCodez gist, claudefa.st, photostructure.com) — all agree on the core fields; the docs page is the authoritative source for the full/current schema.

### Verbatim schema (as published)

```json
{
  "cwd": "/current/working/directory",
  "session_id": "abc123...",
  "session_name": "my-session",
  "prompt_id": "550e8400-e29b-41d4-a716-446655440000",
  "transcript_path": "/path/to/transcript.jsonl",
  "model": {
    "id": "claude-opus-4-8",
    "display_name": "Opus"
  },
  "workspace": {
    "current_dir": "/current/working/directory",
    "project_dir": "/original/project/directory",
    "added_dirs": [],
    "git_worktree": "feature-xyz",
    "repo": {
      "host": "github.com",
      "owner": "anthropics",
      "name": "claude-code"
    }
  },
  "version": "2.1.90",
  "output_style": {
    "name": "default"
  },
  "cost": {
    "total_cost_usd": 0.01234,
    "total_duration_ms": 45000,
    "total_api_duration_ms": 2300,
    "total_lines_added": 156,
    "total_lines_removed": 23
  },
  "context_window": {
    "total_input_tokens": 15500,
    "total_output_tokens": 1200,
    "context_window_size": 200000,
    "used_percentage": 8,
    "remaining_percentage": 92,
    "current_usage": {
      "input_tokens": 8500,
      "output_tokens": 1200,
      "cache_creation_input_tokens": 5000,
      "cache_read_input_tokens": 2000
    }
  },
  "exceeds_200k_tokens": false,
  "effort": { "level": "high" },
  "thinking": { "enabled": true },
  "rate_limits": {
    "five_hour": { "used_percentage": 23.5, "resets_at": 1738425600 },
    "seven_day": { "used_percentage": 41.2, "resets_at": 1738857600 }
  },
  "vim": { "mode": "NORMAL" },
  "agent": { "name": "security-reviewer" },
  "pr": { "number": "... (truncated in source fetch)" },
  "worktree": {
    "name": "my-feature",
    "path": "/path/to/.claude/worktrees/my-feature",
    "branch": "worktree-my-feature"
  }
}
```

Note: the extracted page content truncated mid-object around the `pr`/`review_state` fields (`"...github.com/anthropics/claude-code/pull/1234", "review_state": "pending"`), and the field name is ambiguously `pr` vs. `pull_request` in the raw fetch — **flagged as unconfirmed**; verify directly against `code.claude.com/docs/en/statusline` before relying on the exact PR-related key name in a new implementation.

### Field notes (verbatim/paraphrased from docs)

- `cwd` and `workspace.current_dir` contain the same value; `workspace.current_dir` is the docs' preferred field "for consistency with `workspace.project_dir`."
- `workspace.project_dir`: directory where Claude Code was launched — may differ from `cwd` if the working directory changes mid-session.
- `workspace.added_dirs`: dirs added via `/add-dir` or `--add-dir`; empty array if none.
- `workspace.git_worktree`: worktree name, present only inside a linked `git worktree add` tree, distinct from the `worktree.*` object (which applies only to `--worktree` sessions).
- `workspace.repo.{host,owner,name}`: parsed from the `origin` remote; absent outside a repo or without an `origin` remote.
- `context_window.used_percentage` is calculated **from input tokens only**: `input_tokens + cache_creation_input_tokens + cache_read_input_tokens` — explicitly does **not** include `output_tokens`. The docs warn that manual recalculation from `current_usage` must use this same input-only formula to match `used_percentage`.
- `current_usage` is `null` before the first API call in a session, and again immediately after `/compact` until the next API call repopulates it.
- `effort.level` (reasoning effort) — a community source separately noted this field is a **recent addition**; before it existed, tools like ccusage's statusline had to read effort level from `~/.claude/settings.json` because it wasn't exposed on stdin (there were open GitHub issues requesting exactly this).

### Update/execution mechanics (confirmed, official docs)

- Script runs after each new assistant message, after `/compact` finishes, on permission-mode change, or on vim-mode toggle.
- Updates are **debounced at 300ms** — rapid changes batch together; if a new update fires while the script is still running, the in-flight execution is **cancelled**.
- Triggers can go quiet during idle periods (e.g., a coordinator waiting on background subagents); a `refreshInterval` setting exists specifically to re-run the command on a fixed timer for time-based/external segments.
- Script output: multiple `echo`/`print` lines each become a separate status-line row (multi-line supported natively); ANSI color codes are honored; OSC 8 escape sequences enable clickable links.
- Status line renders in its own row **above** the built-in footer badges — it does not replace them. A separate `footerLinksRegexes` setting exists for adding clickable footer link badges without a custom script.
- A related, separate **`subagentStatusLine`** setting renders a custom row per visible subagent in the agent panel: it runs once per refresh tick with all visible subagent rows in one JSON object on stdin, including base hook fields plus a `columns` field and a `tasks` array (`id, name, type, status, description, label, startTime, model, contextWindowSize, tokenCount, tokenSamples, cwd`).
- Anthropic ships a built-in `/statusline` setup command / skill that generates and wires a script automatically (will overwrite an existing script without asking).

---

## 3. Harness-by-harness statusline hook capability table

| Harness | Native statusline hook? | Input mechanism | Notes |
|---|---|---|---|
| **Claude Code** | Yes — mature, documented | Full JSON on stdin (see schema above); `command`-type entry in `settings.json`; debounced 300ms; optional `refreshInterval` timer | The reference implementation other harnesses are being asked to copy |
| **OpenCode** ([sst/opencode](https://github.com/sst/opencode)) | **No** — feature request only, not yet built ([issue #30295](https://github.com/anomalyco/opencode/issues/30295)) | Proposed design explicitly modeled on Claude Code: `statusLine` config key in `opencode.json`/`tui.json` running a shell command, JSON piped to stdin after each assistant message (`{"model":{...},"session":{...},"context_window":{...},"tokens":{...},"workspace":{...}}`), stdout replaces/extends the status bar | Status bar today is hard-coded in SolidJS with no user customization; custom **system prompts** (not statusline) are supported today via `opencode.json` agent config and markdown instruction files |
| **Crush** ([charmbracelet/crush](https://github.com/charmbracelet/crush)) | **No** — open feature request ([issue #2648](https://github.com/charmbracelet/crush/issues/2648)), plus a separate open request for real-time token-speed display ([issue #3167](https://github.com/charmbracelet/crush/issues/3167)) | None documented; today's status bar shows cumulative token usage + model name, hard-coded | No documented config surface for statusline UI at all as of this research pass |
| **Codex CLI** (OpenAI) | **Partial** — a fixed, built-in picker exists (`[tui].status_line`, `/statusline` command), but **not** a command-backed/custom hook | Built-in enum only: `StatusLineItem` covers Model (ModelName, ModelWithReasoning, Reasoning), Git (GitBranch, PullRequestNumber, BranchChanges), Usage (ContextUsed, ContextRemaining, FiveHourLimit, WeeklyLimit, UsedTokens), System (Permissions, ApprovalMode, CodexVersion, SessionId) | Multiple open feature requests (#17827, #20140, #20244, #16921) explicitly ask for a Claude-Code-style command-backed statusline receiving JSON on stdin; one request (#20244) proposes a `[tui.banner]` config with `command`, `ansi`, `timeout_ms`, `refresh` triggers, and JSON fields incl. `model`, `model_provider`, `cwd`, `session_id`. A separate, broader **hooks framework** exists (PreToolUse, PostToolUse, SessionStart, etc.) but is explicitly turn/session-scoped, not designed for persistent statusline rendering |
| **Gemini CLI** (Google) | Yes — `/footer` (or `/statusline`) command + `settings.json` config | Not a stdin/JSON hook — it's a **declarative item-picker**: choose which built-in items (lines of code, context %, token count, cwd, sandbox status, model+context, vim mode) render in the footer, in what order; a second descriptive header line option exists; footer can be hidden entirely | "Custom items" are described as "coming soon" per a project maintainer's public post — i.e., no external-command execution hook yet, closer to Codex's fixed-enum model than to Claude Code's |
| **Aider** | **No** dedicated statusline feature found | N/A | Shows one-time startup info (model, weak model, git repo) at launch, not a live-updating status bar; customization is oriented around system prompts, coder/edit-format selection (`--edit-format`), and conventions files, not a UI statusline |
| **Cursor CLI** | Yes — `/statusline` command | Community reports (forum + third-party skill write-ups) describe it as **explicitly compatible with Claude Code's implementation** — i.e., appears to consume the same/similar stdin JSON shape and script-command model | Cursor CLI/desktop also has a broader **hooks system** (stdio JSON both directions, `hooks.json` at project `.cursor/hooks/` or user `~/.cursor/`) that reportedly **can load hooks from third-party tools including Claude Code**, meaning Claude-Code-style statusline scripts may be directly portable to Cursor without modification — the strongest evidence found for one binary serving 2+ harnesses today |

**Harness-independence implication (synthesis, not a single-source claim):** Only **Claude Code** and **Cursor CLI** currently expose a genuine command-backed stdin-JSON statusline hook, and Cursor's is explicitly designed for Claude Code compatibility. **Codex CLI** and **Gemini CLI** offer fixed-enum, declarative footer/statusline pickers (no external command execution) — a binary cannot inject into these today; it could at most influence which built-in items are chosen. **OpenCode** and **Crush** have zero live statusline customization today, only open feature requests that name-check Claude Code's model as the design template. **Aider** has no statusline concept at all. Practically: a new binary's stdin-JSON contract should be a **superset-compatible** design that mirrors Claude Code's schema (since Cursor already piggybacks on it and OpenCode's own proposal explicitly copies it) — this maximizes the number of harnesses it can serve today (Claude Code, Cursor) and positions it to be adoptable the moment OpenCode/Codex/Crush ship their pending hook proposals, since all of their public proposals cite the Claude Code stdin-JSON model as the reference design.

---

## 4. Design & performance best-practices synthesis (from starship, oh-my-posh, powerlevel10k/gitstatusd, spaceship, tmux)

### Speed architecture patterns
- **Compiled binary vs. cold interpreter start dominates the perf story.** Starship (Rust) and Oh My Posh (Go) both cite 5-15ms full-render times; Powerlevel10k with gitstatusd is in the same ballpark; the Claude Code ecosystem shows the same divide (Node.js cold-start ~45-120ms typical vs. Go/Rust rewrites at single-digit-ms or microseconds). **Conclusion for a new binary: ship a compiled, single-binary artifact (Rust or Go), not an interpreted-language cold-start CLI.**
- **Parallel/async segment execution.** Starship runs modules in parallel via Rust async; Oh My Posh executes segments concurrently via goroutines and supports a "streaming" mode where the prompt renders immediately and slow segments are patched in as they complete (placeholder + background re-render via channel); Spaceship (zsh) uses `zsh-async` so slow segments show an ellipsis placeholder instead of blocking the whole prompt. **Pattern: never let one slow segment (e.g., a network call) block the entire line — render fast segments immediately, patch slow ones asynchronously.**
- **Explicit timeout controls.** Starship exposes `scan_timeout` (context detection, e.g., "is this a git repo") and `command_timeout` (per-module execution cap) so no single module can make the whole prompt feel laggy.
- **Caching with TTL.** Oh My Posh supports per-segment `cache_duration` (e.g., `10m`) keyed by a `CacheKey`, stored at session level — though a documented bug shows cache-duration semantics need careful, precise definition (whether TTL resets on each render) to avoid serving stale data (e.g., stale git branch after a checkout). tmux status-bar authors independently converge on the same pattern: a generic TTL-cache wrapper around expensive `#()` commands, combined with **cache-key includes the current git SHA** (rather than purge-on-checkout) as a "clever" invalidation strategy, plus firing `tmux refresh-client -S` from a git `post-checkout` hook to force-refresh on branch change instead of waiting out the TTL.
- **Instant/two-phase startup.** Powerlevel10k's "Instant Prompt" shows a basic prompt immediately at shell startup while slow plugins/segments finish loading in a second phase — directly analogous to Oh My Posh's streaming-placeholder approach.
- **Avoid subprocess spawns; use library calls where possible.** A documented Starship optimization case study: replacing subprocess-based `git status`/`git branch`/`git stash list` calls with direct `libgit2` calls via the `git2` crate cut a custom git segment from 52ms to 25ms; a further fix removed two redundant `git rev-parse --is-inside-work-tree` subprocess calls (~3ms each) that were redundant with a discovery already done via `libgit2`, saving ~6ms/prompt with zero visual change. **Pattern: every subprocess spawn avoided is a measurable, stackable win; consolidate all git-state detection into a single library call/daemon round-trip instead of N separate `git` invocations.**

### Cheap, frequent git status (gitstatusd deep-dive — the strongest evidence base found)
- **gitstatusd (romkatv)** is a C++ daemon used by Powerlevel10k; it operates as a persistent client/server: the daemon reads `{id, directory}` requests from stdin and prints `{id, status}` responses to stdout; shell bindings spawn it once in the background and pipe requests to it per-prompt, avoiding the ~20ms+ process-startup cost of invoking a fresh `git` binary every prompt (author explicitly notes process invocation is markedly slower on macOS than Linux).
- **Why it's fast is NOT "it's a daemon with a hot cache that skips work"** — the author explicitly debunks that framing. In an apples-to-apples benchmark (clean Chromium repo) it beats `libgit2`'s `git_diff_index_to_workdir` (the single most expensive part of a status computation) by **46.3x**, decomposed into three independently-measured sources: (1) more efficient in-memory data structures/algorithms → 32x less userspace CPU time, (2) fewer & cheaper syscalls → 1.9x less kernel CPU time, (3) multi-threaded parallel scanning of index + workdir with near-perfect core scaling → 12.4x less wall-clock time (roughly orthogonal to the CPU-time wins). Absolute numbers: gitstatusd 30.9ms (hot) vs. plain `git` 295ms vs. unpatched `libgit2` 1310ms for status; `describe` at 0.0345ms vs. 14.5ms vs. 45.2ms respectively `[romkatv/gitstatus README + Lobsters discussion, consistent across both]`.
- **Real shortcuts gitstatusd *does* use on top of the raw-speed algorithm** (separate from the benchmark numbers above): because a status prompt segment usually only needs to know **whether** there are staged/unstaged/untracked changes (not enumerate every file), it can terminate its workdir scan **early** on first match; it also remembers which files were dirty on the previous run and re-checks those first, so a repeatedly-dirty repo can often skip a full scan entirely on subsequent prompts.
- **Untracked-cache-style memoization:** gitstatusd remembers each directory's last-modified time plus its list of untracked files, so on the next scan it can skip re-listing a directory whose mtime hasn't changed — explicitly analogous to (and inspired by) git's own untracked-cache extension, but romkatv notes git's own untracked-cache documentation overstates its benefit: it cannot eliminate the need to `stat()` every indexed file, only directory-listing.
- **Known correctness trade-off:** gitstatusd depends on a patched `libgit2`, which has gaps vs. real git (e.g., no support for `skipHash`) — a source of prompt/reality discrepancies in edge cases. **Design implication for a new tool: a "fast but occasionally slightly wrong" git-status engine is an accepted, well-precedented trade-off in this space, but should be documented and ideally detectable/fallback-able.**
- **`git status --porcelain=v2` is the correct scripting/parsing target** (vs. the human-readable long format, which is explicitly *not* guaranteed stable across git versions). Recommended invocation for prompt/status use: `git --no-optional-locks status --porcelain=v2 --branch -uno` — `--no-optional-locks` avoids lock contention with foreground git commands, `-uno` skips the (expensive) untracked-file scan for speed, `--branch` adds the `branch.ab` ahead/behind line. Minimum git version for `--porcelain=v2 --branch` support: v2.13.2 (per the third-party `porcelain` Go tool's stated requirement).
- **Monorepo/large-repo pitfall:** unfiltered `node_modules`-style directories not covered by `.gitignore` can make even `git status` itself grind to a halt; mitigations found: ensure `.gitignore` coverage at repo root, enable git's untracked cache and FSMonitor (both have a "warm-up" cost — first few runs after enabling are *not* faster, subsequent runs are), and pass `-uno` in automated/scripted contexts.

### Config formats
- **Starship: TOML-only** (`~/.config/starship.toml`, overridable via `STARSHIP_CONFIG`); one table per module (`[module_name]`); requires escaping of `$ [ ] ( )` in format strings.
- **Oh My Posh: format-agnostic** — supports JSON (default), YAML, and TOML for the same schema, plus a maintained JSON Schema for editor autocomplete/validation; config can be a local path or a remote URL. Cross-format converters exist but are documented as imperfect.
- **claude-powerline: JSON**, with a 4-tier location/precedence system (CLI flag > env var > config file > default) and three possible file locations (project, user `~/.claude/`, XDG `~/.config/`).
- **ccstatusline: JSON** at `~/.config/ccstatusline/settings.json`, override via `--config`.

### Theming / glyphs
- **Nerd Fonts** is the de facto standard icon substrate for this entire tool category (Starship, Powerlevel10k, Oh My Posh, and the Claude Code tools all recommend/require a Nerd-Font-patched font, e.g., Meslo/FiraCode Nerd Font). Mechanism: Nerd Fonts patches ~10,000+ glyphs from Font Awesome, Devicons, Octicons, Powerline (+ `powerline-extra-symbols`), Material Design Icons, etc. into the font's Private-Use-Area Unicode code points; any renderer emitting those code points displays the icon if the active font has the glyph mapped.
- **Powerline separators** are specific PUA code points (e.g., `U+E0B4`/`U+E0B6` for rounded left/right separators) — this is the shared visual vocabulary across every powerline-style tool surveyed, including ccstatusline and claude-powerline in the Claude Code space.
- Graceful degradation is expected/standard: claude-powerline documents a `--charset=text` ASCII-only fallback mode for users without Nerd Fonts; `NO_COLOR` / `FORCE_COLOR` env-var conventions are respected across ccstatusline, claude-powerline, and cc-statusline.

### Segment model
- Universally, these tools converge on a **segment/module/widget** abstraction: an ordered list of independently toggleable, independently colored/styled units (Starship "modules", Oh My Posh "segments" inside "blocks", ccstatusline/claude-powerline "widgets"), each optionally wrapped in powerline caps/separators, with a global fallback/flex-separator mechanism to adapt to terminal width (ccstatusline's "Smart Width Detection", claude-powerline's responsive `breakpoints` in TUI grid mode).

---

## 5. Gaps / opportunities for a new, ground-up statusline binary

1. **No compiled, harness-independent binary currently owns both "fast" and "rich/themed."** The rich-feature tools (ccstatusline, claude-powerline) are Node/TS with real cold-start cost; the fast tools (claude-statusline/Go, CCometixLine/Rust, ccusage-statusline-rs) are comparatively feature-thin (mostly single-line, fewer widgets, less theming depth). A single compiled binary matching ccstatusline's widget/theme/multi-line breadth at Go/Rust startup speed is an open gap.

2. **Harness portability is currently accidental, not designed-for.** Every tool surveyed is built specifically against the Claude Code stdin-JSON schema. Only Cursor CLI is confirmed to consume a compatible-enough shape to reuse Claude-Code scripts unmodified; Codex, Gemini CLI, OpenCode, and Crush each have their own (partial or nonexistent) mechanism. A new binary designed with (a) an abstract internal data model, (b) pluggable input adapters (Claude Code JSON, Codex's fixed-enum equivalents, a generic env-var fallback, a "poll external state myself" mode for harnesses with zero hook support), and (c) a single rendering/theming core, would be structurally unique — nothing surveyed does this today.

3. **Process-spawn safety is an unsolved, recurring failure mode.** ccusage's statusline has two open, reproduced GitHub issues (#455, #459) about runaway process spawning / 100%+ CPU from naive re-invocation under Claude Code's re-run-on-update model; cc-statusline had to bolt on an explicit file-based locking mechanism as a workaround. A ground-up binary should treat "safe under rapid re-invocation" (debounce-aware, single-instance-locking, sub-10ms typical execution so re-entrancy is rarely even possible) as a first-class design requirement, not an afterthought.

4. **Git-status performance technique (gitstatusd-style) has not been adopted by any Claude Code statusline tool surveyed.** All the Claude Code tools examined appear to shell out to `git` directly (subprocess-per-render) rather than using a persistent daemon or a `libgit2`-linked in-process call — the single biggest, best-evidenced perf lever found in this research (up to 46x on the git-status computation itself, per gitstatusd's own benchmarks) is currently unexploited in this specific tool category. A new binary using direct `libgit2`/`git2`-crate bindings (or a lightweight embedded gitstatusd-equivalent) with `--porcelain=v2`-equivalent semantics and early-exit-on-dirty logic would be a clear differentiator.

5. **Correctness-vs-speed trade-off documentation is inconsistent.** gitstatusd's known libgit2 gaps (e.g., `skipHash`) are openly documented upstream; none of the Claude Code statusline tools surveyed document equivalent caveats for their own git-status shortcuts (most are opaque about exactly how they compute git state). A new tool with an explicit, documented "fast path may occasionally show slightly stale info; here's when and how to force a full rescan" contract would be a trust/transparency differentiator over the status quo.

6. **Async/streaming segment rendering (Starship/Oh My Posh/Spaceship pattern) is not observed in any surveyed Claude Code statusline tool**, all of which appear to compute-then-print synchronously within Claude Code's 300ms debounce window. Given Claude Code's own hard requirement (must print to stdout and exit; no persistent process), a genuinely async model isn't directly portable, but a **fast local cache + background refresh daemon** (write results to a temp file/socket asynchronously, have the statusLine invocation just read the last-known-good cached value in <1ms) is an available adaptation of the same idea, not yet seen in this ecosystem.

7. **Config format fragmentation** (JSON-only across all Claude Code tools vs. Starship's TOML / Oh My Posh's tri-format) suggests no strong consensus exists in this sub-category — an opportunity to pick whichever format best serves a target user base (TOML for parity with Starship/dotfile users who already template TOML; JSON for parity with existing Claude Code tool configs) without contradicting any established norm.

---

## Known unknowns / unresolved items

| Question | Why unresolved | Tried |
|---|---|---|
| Exact key name for the PR/pull-request field in Claude Code's stdin schema (`pr` vs. `pull_request`) and its full sub-field list | The official docs page content was truncated mid-object during the automated fetch/index pass right at this field | Source: `https://code.claude.com/docs/en/statusline` (fetched via ctx_fetch_and_index, section boundary cut the JSON object); queries: "effort field pull_request worktree review_state hook_event_name schema tail" |
| Definitive current npm download counts / GitHub star counts for each tool (used only as rough popularity signal, not a hard datum in the tables above) | Search-engine snapshots return point-in-time numbers with no independent re-verification; treated as `[single-source]` and excluded from any claim needing 2x confirmation | Queries: "ccstatusline npm github 2026", "claude-powerline Owloops github npm statusline" |
| Whether Cursor CLI's `/statusline` genuinely consumes byte-identical JSON to Claude Code's schema, or a compatible superset/subset | Evidence is from a forum post and a third-party "skill" description, not Cursor's own official schema documentation | Query: "Cursor CLI statusline customization hook"; tried fetching Cursor's own docs page reference (`cursor.com/docs/hooks`) only at search-summary level, not deep-fetched this pass |
| Precise current status (merged/open) of OpenCode issue #30295, Crush issues #2648/#3167, Codex issues #17827/#20140/#20244/#16921 | These are live, evolving GitHub issues; this research captured their state as of the search-engine snapshot only, not by directly fetching each issue's current comment thread | Not deep-fetched; would require issue-by-issue WebFetch to confirm merge status at time of use |
| Exact benchmark methodology behind the various self-reported perf numbers (ccusage-statusline-rs "15x/8ms vs 120ms", claude-statusline-Go "19µs", cc-statusline "45-80ms", claude-code-statusline "<50ms mean") | All are author-self-reported in READMEs with no independently reproduced benchmark found across sources | Flagged `[single-source]` inline in Section 1 rather than presented as verified fact |

---

## Sources (deduplicated)

- [Claude Code statusline docs (official)](https://code.claude.com/docs/en/statusline)
- [sirmalloc/ccstatusline](https://github.com/sirmalloc/ccstatusline)
- [ccstatusline on npm](https://www.npmjs.com/package/ccstatusline)
- [Owloops/claude-powerline](https://github.com/Owloops/claude-powerline)
- [claude-powerline README](https://github.com/Owloops/claude-powerline/blob/main/README.md)
- [@owloops/claude-powerline on npm](https://www.npmjs.com/package/@owloops/claude-powerline)
- [chongdashu/cc-statusline](https://github.com/chongdashu/cc-statusline)
- [@chongdashu/cc-statusline on npm](https://www.npmjs.com/package/@chongdashu/cc-statusline)
- [ccusage statusline guide](https://ccusage.com/guide/statusline)
- [ryoppippi/ccusage](https://github.com/ryoppippi/ccusage)
- [ccusage issue #455 (OOM/cache)](https://github.com/ryoppippi/ccusage/issues/455)
- [ccusage issue #459 (infinite spawning loop)](https://github.com/ryoppippi/ccusage/issues/459)
- [ticpu/ccusage-statusline-rs](https://github.com/ticpu/ccusage-statusline-rs)
- [Haleclipse/CCometixLine](https://github.com/Haleclipse/CCometixLine)
- [MaurUppi/CCstatus](https://github.com/MaurUppi/CCstatus)
- [TheoBrigitte/claude-statusline](https://github.com/TheoBrigitte/claude-statusline)
- [sotayamashita/claude-code-statusline](https://github.com/sotayamashita/claude-code-statusline)
- [Yiğit Konur — Claude Code Statuslines Compared](https://yigitkonur.com/research/claude-code-statuslines-compared)
- [AKCodez gist — Claude Code Status Line complete field guide](https://gist.github.com/AKCodez/ffb420ba6a7662b5c3dda2edce7783de)
- [claudefa.st — Claude Code Status Line Setup Guide](https://claudefa.st/blog/tools/statusline-guide)
- [PhotoStructure — Custom status lines for Claude Code](https://photostructure.com/coding/claude-code-statusline/)
- [OpenCode issue #30295 — custom statusLine feature request](https://github.com/anomalyco/opencode/issues/30295)
- [opencode.ai CLI docs](https://opencode.ai/docs/cli/)
- [Crush issue #2648 — status line customization](https://github.com/charmbracelet/crush/issues/2648)
- [Crush issue #3167 — token speed in status bar](https://github.com/charmbracelet/crush/issues/3167)
- [charmbracelet/crush](https://github.com/charmbracelet/crush)
- [Codex issue #17827 — customizable status line](https://github.com/openai/codex/issues/17827)
- [Codex issue #20140 — command-backed statusline](https://github.com/openai/codex/issues/20140)
- [Codex issue #20244 — custom command-backed TUI status line](https://github.com/openai/codex/issues/20244)
- [Codex issue #16921 — custom status line plugins](https://github.com/openai/codex/issues/16921)
- [Codex CLI reference (developers.openai.com)](https://developers.openai.com/codex/cli/reference)
- [Codex hooks docs](https://developers.openai.com/codex/hooks)
- [Gemini CLI configuration docs](https://google-gemini.github.io/gemini-cli/docs/get-started/configuration.html)
- [Gemini CLI issue #8191 — Statusline](https://github.com/google-gemini/gemini-cli/issues/8191)
- [Gemini CLI issue #21208 — status line review polish](https://github.com/google-gemini/gemini-cli/issues/21208)
- [Aider documentation](https://aider.chat/docs/)
- [Cursor forum — Configurable Status Lines in Cursor Agent](https://forum.cursor.com/t/configurable-status-lines-in-cursor-agent/152287)
- [Cursor hooks docs](https://cursor.com/docs/hooks)
- [Starship](https://starship.rs/config/)
- [Starship advanced config](https://starship.rs/advanced-config/)
- [Starship GitHub issue #597 — slow on large HDD checkouts](https://github.com/starship/starship/issues/597)
- [dasroot.net — Inside Starship](https://dasroot.net/posts/2026/03/inside-starship-customizing-prompt-rust/)
- [Matt Michie — Migrating from Powerline-Go to Starship](https://mattmichie.com/2026/02/14/migrating-from-powerline-go-to-starship/)
- [Oh My Posh](https://ohmyposh.dev/)
- [Oh My Posh general config docs](https://ohmyposh.dev/docs/configuration/general)
- [Oh My Posh issue #6011 — git segment caching duration](https://github.com/JanDeDobbeleer/oh-my-posh/issues/6011)
- [romkatv/powerlevel10k](https://github.com/romkatv/powerlevel10k)
- [powerlevel10k gitstatus README](https://github.com/romkatv/powerlevel10k/blob/master/gitstatus/README.md)
- [romkatv/gitstatus](https://github.com/romkatv/gitstatus)
- [gitstatus README](https://github.com/romkatv/gitstatus/blob/master/README.md)
- [Lobsters — 10x faster implementation of Git status](https://lobste.rs/s/nyuvvt/10x_faster_implementation_git_status)
- [spaceship-prompt.sh config docs](https://spaceship-prompt.sh/config/prompt/)
- [spaceship-prompt/spaceship-prompt](https://github.com/spaceship-prompt/spaceship-prompt)
- [tao-of-tmux status bar docs](https://tao-of-tmux.readthedocs.io/en/latest/manuscript/09-status-bar.html)
- [semanticart.com — CI Results in Your tmux Status Line](https://blog.semanticart.com/2020/02/13/ci-results-in-your-tmux-status-line/)
- [nerdfonts.com](https://www.nerdfonts.com/)
- [ryanoasis/nerd-fonts](https://github.com/ryanoasis/nerd-fonts)
- [ryanoasis/powerline-extra-symbols](https://github.com/ryanoasis/powerline-extra-symbols)
- [Eloquent Coder — Complete Guide to git status](https://medium.com/@eloquentcoder/the-complete-guide-to-git-status-from-your-first-check-to-advanced-scripting-215450c87939)
- [nushell discussion #17003 — git status --porcelain=v2 parsing](https://github.com/nushell/nushell/discussions/17003)
- [robertgzr/porcelain](https://github.com/robertgzr/porcelain)
