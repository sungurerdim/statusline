# statusline

## Blueprint Profile

Type: cli | Stack: rust-2024 | Target: production
Mission: give a developer using an AI coding harness one always-correct status block — live git state plus the harness's own session numbers — from a single binary with no runtime to install
Priorities: render-speed, numeric-fidelity, harness-independence, zero-runtime-footprint
Constraints: keep-rust, minimal-deps, single-static-binary, no-config-required
Red lines: no-network, no-disk-writes, no-invented-values, no-runtime-dependency, no-cost-estimation-tables
Integrations: none
Data: none-persisted | Regulations: none
Audience: public | Deploy: binary-copy + prebuilt-release-artifacts (macos+linux+windows)

Entry: src/main.rs (std-only, no framework)
Modules: src/input=harness-json-normalize(2); src/git=subprocess-git-state(1); src/render=segment-build+layout(1); src/theme=color+glyph-detect(1); src/config=toml+env-precedence(1); src/model=normalized-types(1)
Data Flow: harness-stdin-json→adaptive-parse→git-subprocess→segment-render→stdout
External: serde(derive); serde_json(harness payload); toml(config); git(subprocess — sole state source)
Toolchain: cargo fmt+clippy+test | CI: github-actions(ci+audit) | Container: none

Ideal: coupling=40 cohesion=75 complexity=10 coverage=70

Scores: sec=99 quality=100 arch=100 perf=97 resil=95 test=99 stack=100 dx=99 docs=100 overall=99 model=claude-opus-5

## End Blueprint Profile
