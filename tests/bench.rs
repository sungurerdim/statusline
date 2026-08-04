//! Render benchmark — the source of the numbers quoted in README.md.
//!
//! Run it explicitly (it is `#[ignore]`d so `cargo test` stays fast):
//!
//! ```text
//! cargo test --release --test bench -- --ignored --nocapture
//! ```
//!
//! Written as a test rather than a shell script so it runs on every platform
//! the project supports — a Windows contributor can reproduce the README's
//! figures without a POSIX shell.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_statusline")
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git must be on PATH");
    assert!(out.status.success(), "git {args:?} failed");
}

/// A 10-file repo; `dirty` leaves tracked edits plus an untracked file, which
/// is the worst case (two git subprocesses per render).
fn repo(name: &str, dirty: bool) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("statusline-bench-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]);
    git(&dir, &["config", "user.email", "bench@example.com"]);
    git(&dir, &["config", "user.name", "Bench"]);
    for i in 0..10 {
        std::fs::write(dir.join(format!("f{i}.txt")), "line\nline\nline\n").unwrap();
    }
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "init"]);
    if dirty {
        std::fs::write(dir.join("f0.txt"), "changed\nline\nline\n").unwrap();
        std::fs::write(dir.join("f1.txt"), "line\nline\nline\nextra\n").unwrap();
        std::fs::write(dir.join("new.txt"), "untracked\n").unwrap();
    }
    dir
}

fn payload(dir: &Path) -> String {
    // `\` is a JSON escape and Windows paths are backslash-separated.
    let path = dir.to_str().unwrap().replace('\\', "\\\\");
    format!(
        r#"{{"cwd":"{path}","model":{{"display_name":"Claude Opus 5"}},
"workspace":{{"current_dir":"{path}","repo":{{"name":"repo"}}}},
"cost":{{"total_cost_usd":0.42,"total_duration_ms":7200000,"total_api_duration_ms":3600000}},
"context_window":{{"context_window_size":1000000,"used_percentage":17,
"current_usage":{{"input_tokens":8500,"cache_read_input_tokens":161500}}}},
"rate_limits":{{"five_hour":{{"used_percentage":23.5}},"seven_day":{{"used_percentage":2}}}}}}"#
    )
}

fn render_once(dir: &Path, payload: &str) {
    let mut child = Command::new(bin())
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.as_bytes())
        .unwrap();
    child.wait().unwrap();
}

fn measure(label: &str, dir: &Path, runs: usize) {
    let p = payload(dir);
    for _ in 0..5 {
        render_once(dir, &p); // warm the loader and git's object cache
    }
    let mut samples: Vec<f64> = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t0 = Instant::now();
        render_once(dir, &p);
        samples.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let at = |q: f64| samples[((samples.len() as f64 * q) as usize).min(samples.len() - 1)];
    println!(
        "{label:12} runs={runs}  min={:.2}ms  p50={:.2}ms  p90={:.2}ms  max={:.2}ms",
        samples[0],
        at(0.50),
        at(0.90),
        samples[samples.len() - 1]
    );
}

#[test]
#[ignore = "benchmark — run explicitly with --ignored"]
fn render_benchmark() {
    let runs = std::env::var("BENCH_RUNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(80);

    let dirty = repo("dirty", true);
    let clean = repo("clean", false);
    measure("dirty tree", &dirty, runs);
    measure("clean tree", &clean, runs);

    let size = std::fs::metadata(bin()).unwrap().len();
    println!("binary       {size} bytes");

    let _ = std::fs::remove_dir_all(&dirty);
    let _ = std::fs::remove_dir_all(&clean);
}
