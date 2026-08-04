//! End-to-end tests: run the real binary, feed it stdin, assert on stdout.
//!
//! The unit tests cover each layer in isolation; these cover the wiring in
//! `main.rs` that no unit test touches — stdin read, config load, git collect,
//! render, print — which is exactly the path a harness exercises.

use std::io::Write;
use std::process::{Command, Stdio};

/// Path to the binary under test, provided by cargo for integration tests.
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_statusline")
}

/// Run the binary in `cwd` with `stdin`, returning stdout. Colour is disabled
/// so assertions match plain text rather than SGR sequences.
fn run(stdin: &str, cwd: &std::path::Path) -> String {
    let mut child = Command::new(bin())
        .current_dir(cwd)
        .env("NO_COLOR", "1")
        .env("STATUSLINE_GLYPHS", "ascii")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary must start");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("binary must exit");
    assert!(out.status.success(), "exit status {:?}", out.status);
    String::from_utf8(out.stdout).expect("stdout must be utf-8")
}

const CLAUDE_CODE_PAYLOAD: &str = r#"{
  "cwd": "PLACEHOLDER",
  "model": { "id": "claude-opus-5", "display_name": "Claude Opus 5" },
  "workspace": { "current_dir": "PLACEHOLDER", "repo": { "name": "fixture" } },
  "effort": { "level": "high" },
  "cost": { "total_cost_usd": 0.42, "total_duration_ms": 7200000, "total_api_duration_ms": 3600000 },
  "context_window": {
    "context_window_size": 1000000,
    "used_percentage": 17,
    "current_usage": { "input_tokens": 8500, "cache_creation_input_tokens": 5000, "cache_read_input_tokens": 156500 }
  },
  "rate_limits": { "five_hour": { "used_percentage": 23.5 }, "seven_day": { "used_percentage": 2 } }
}"#;

/// A git repo with one committed file, modified in the working tree.
///
/// The directory name carries the pid so two concurrent runs on one machine
/// (a second `cargo test`, or a coverage run alongside it) cannot race on the
/// same path — the fixture starts by deleting it.
fn fixture(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("statusline-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap()
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "test@example.com"]);
    git(&["config", "user.name", "Test User"]);
    std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "init"]);
    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    dir
}

#[test]
fn renders_every_harness_field_end_to_end() {
    let dir = fixture("full");
    // Windows paths are backslash-separated and `\` is a JSON escape, so an
    // unescaped path silently produces malformed JSON — the tool then falls
    // back to a git-only render and every field assertion below fails for the
    // wrong reason. Escape before substituting.
    let path = dir.to_str().unwrap().replace('\\', "\\\\");
    let payload = CLAUDE_CODE_PAYLOAD.replace("PLACEHOLDER", &path);
    let out = run(&payload, &dir);

    assert!(out.contains("fixture:"), "repo name missing: {out}");
    assert!(out.contains("Opus 5"), "model missing: {out}");
    assert!(out.contains("high"), "effort missing: {out}");
    assert!(out.contains("170K"), "context tokens missing: {out}");
    assert!(out.contains("17%"), "context percent missing: {out}");
    assert!(out.contains("$0.42"), "cost missing: {out}");
    // 0.42 over 1h of API time (not the 2h wall clock)
    assert!(
        out.contains("0.42/h"),
        "burn rate missing or wall-based: {out}"
    );
    assert!(out.contains("5h 24%"), "5h window missing: {out}");
    assert!(out.contains("7d 2%"), "7d window missing: {out}");
    assert!(out.contains("+1"), "uncommitted line diff missing: {out}");
    assert!(!out.contains('\x1b'), "NO_COLOR must suppress SGR: {out}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn renders_git_only_line_without_harness_input() {
    let dir = fixture("bare");
    let out = run("", &dir);
    assert!(
        out.contains("statusline-cli-bare"),
        "repo line missing: {out}"
    );
    assert!(
        !out.contains('$'),
        "no cost should appear without a harness: {out}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn malformed_stdin_still_renders_rather_than_failing() {
    let dir = fixture("garbage");
    let out = run("not json at all {{{", &dir);
    assert!(!out.trim().is_empty(), "must still render something");
    assert!(
        out.contains("statusline-cli-garbage"),
        "git line missing: {out}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn runs_outside_a_git_repo() {
    let dir = std::env::temp_dir().join(format!("statusline-cli-nogit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = run("{}", &dir);
    assert!(out.contains("no git"), "expected the no-git marker: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}
