#!/usr/bin/env bash
# Reproducible render benchmark — the numbers in README.md come from this.
#
#   ./scripts/bench.sh [runs]
#
# Builds the release binary, creates a throwaway git repo with a dirty working
# tree (the worst case: two git subprocesses), and reports the wall-time
# distribution plus the binary size. Nothing outside $TMPDIR is touched.
set -euo pipefail

RUNS="${1:-80}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

cargo build --locked --release --manifest-path "$ROOT/Cargo.toml" >/dev/null
BIN="$ROOT/target/release/statusline"

REPO="$WORK/repo"
mkdir -p "$REPO"
git -C "$REPO" init -q
git -C "$REPO" config user.email bench@example.com
git -C "$REPO" config user.name Bench
for i in $(seq 1 10); do
  printf 'line %s\nline %s\nline %s\n' "$i" "$i" "$i" >"$REPO/f$i.txt"
done
git -C "$REPO" add -A
git -C "$REPO" commit -qm init
printf 'changed\nline 1\nline 1\n' >"$REPO/f1.txt"
printf 'changed\n' >>"$REPO/f2.txt"
echo untracked >"$REPO/new.txt"

cat >"$WORK/payload.json" <<'JSON'
{"cwd":"REPO_PATH","model":{"id":"claude-opus-5","display_name":"Claude Opus 5"},
"workspace":{"current_dir":"REPO_PATH","repo":{"name":"repo"}},
"cost":{"total_cost_usd":0.42,"total_duration_ms":7200000,"total_api_duration_ms":3600000},
"context_window":{"context_window_size":1000000,"used_percentage":17,
"current_usage":{"input_tokens":8500,"cache_creation_input_tokens":5000,"cache_read_input_tokens":156500}},
"rate_limits":{"five_hour":{"used_percentage":23.5},"seven_day":{"used_percentage":2}}}
JSON
sed -i.bak "s|REPO_PATH|$REPO|g" "$WORK/payload.json" && rm -f "$WORK/payload.json.bak"

python3 - "$BIN" "$REPO" "$WORK/payload.json" "$RUNS" <<'PY'
import subprocess, sys, time

binary, repo, payload_path, runs = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4])
payload = open(payload_path, "rb").read()

for _ in range(5):  # warm dyld + git object cache
    subprocess.run([binary], input=payload, cwd=repo,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

samples = []
for _ in range(runs):
    t0 = time.perf_counter()
    subprocess.run([binary], input=payload, cwd=repo,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    samples.append((time.perf_counter() - t0) * 1000)

samples.sort()
pick = lambda q: samples[min(len(samples) - 1, int(len(samples) * q))]
print(f"runs={runs}  min={samples[0]:.2f}ms  p50={pick(0.50):.2f}ms  "
      f"p90={pick(0.90):.2f}ms  max={samples[-1]:.2f}ms")
PY

echo "binary: $(wc -c <"$BIN" | tr -d ' ') bytes"
