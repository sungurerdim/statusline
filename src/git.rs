//! Git state via a single subprocess.
//!
//! The old implementation spawned 3-4 `git` processes per render (`status`,
//! `rev-parse --show-toplevel`, `describe`). We collapse the common case to
//! **one** call:
//!
//! ```text
//! git --no-optional-locks status --porcelain=v2 --branch --show-stash
//! ```
//!
//! - `--no-optional-locks` avoids lock contention with a foreground `git`.
//! - `--porcelain=v2` is the stable, machine-parseable format (long format is
//!   explicitly *not* stability-guaranteed across git versions).
//! - `--branch` adds `# branch.*` header lines (head, upstream, ahead/behind).
//! - `--show-stash` adds the `# stash N` line.
//!
//! Repo name comes from the harness input (no extra spawn); the optional
//! release tag is gated behind config (off by default) because it needs a
//! second `git describe` spawn.

use crate::model::GitInfo;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Ceiling on any single `git` subprocess — well above the ~6ms typical call,
/// but bounded so a hung/blocked process (lock contention, a repo on a stalled
/// network mount, ...) can't stall the statusline indefinitely.
const GIT_TIMEOUT: Duration = Duration::from_secs(2);

/// Spawn `program` detached from our stdio, ignoring the result. Used only to
/// terminate a stalled child, where there is nothing useful to do on failure.
fn run_detached(program: &str, args: &[&str]) {
    let _ = Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Terminate a stalled child by pid, using the platform's own tool so we stay
/// dependency-free. `Child::kill()` is not reachable here — the waiter thread
/// owns the `Child` — so the pid is signalled directly instead.
#[cfg(unix)]
fn terminate(pid: u32) {
    run_detached("kill", &["-TERM", &pid.to_string()]);
}

/// `/T` also takes the process tree: `git` may have spawned a pager or helper.
#[cfg(windows)]
fn terminate(pid: u32) {
    run_detached("taskkill", &["/PID", &pid.to_string(), "/T", "/F"]);
}

#[cfg(not(any(unix, windows)))]
fn terminate(_pid: u32) {}

/// Run `cmd` and return its output, or `None` if it doesn't finish within
/// `timeout` (the child is signalled before we give up).
///
/// A worker thread does the blocking `wait_with_output()` and the caller waits
/// on a channel. Two reasons this beats the obvious `try_wait()` poll loop:
///
/// - **No latency quantum.** Polling on a fixed 5ms sleep rounded every git
///   call up to the next 5ms boundary; a 6ms `git status` cost 10ms. Blocking
///   returns the instant the child does.
/// - **No pipe deadlock.** `wait_with_output()` drains stdout/stderr while it
///   waits. A poll loop never reads the pipes, so any git command producing
///   more than the ~64 KiB pipe buffer (e.g. `status -uall` in a large tree)
///   blocked forever on write and was killed at the timeout — the statusline
///   silently lost git state exactly on the repos where it matters most.
fn run_with_timeout(cmd: &mut Command, timeout: Duration) -> Option<Output> {
    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    // Kept for the timeout path: the worker still owns the `Child` (so it is
    // unreaped and the pid cannot have been recycled) but we can no longer
    // call `.kill()` on it from here.
    let pid = child.id();

    let (tx, rx) = mpsc::channel();
    // 64 KiB stack: this thread only blocks on wait and sends the result.
    if thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(move || {
            let _ = tx.send(child.wait_with_output());
        })
        .is_err()
    {
        // The closure (and with it the `Child`) is gone, so the only handle
        // left is the pid. Signal it rather than abandoning a live git.
        terminate(pid);
        return None;
    }

    match rx.recv_timeout(timeout) {
        Ok(Ok(out)) => Some(out),
        // Timed out or the wait itself failed. Signal the child so a stalled
        // git can't accumulate across renders (we run once every refresh).
        _ => {
            terminate(pid);
            None
        }
    }
}

/// Options controlling the git query (wired to config — see `Config::git_options`).
#[derive(Debug, Clone, Copy)]
pub struct GitOptions {
    /// `"normal"` (default), `"no"` (skip untracked scan — fastest), `"all"`.
    pub untracked: UntrackedMode,
    /// Also run `git describe` for the latest tag (extra spawn).
    pub tags: bool,
    /// Whether the caller renders the uncommitted line diff. When off we skip
    /// the `git diff --numstat` spawn entirely — it is pure waste otherwise,
    /// and it is the single most expensive thing we do after `status`.
    pub lines: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UntrackedMode {
    No,
    Normal,
    All,
}

impl Default for GitOptions {
    fn default() -> Self {
        Self {
            untracked: UntrackedMode::Normal,
            tags: false,
            lines: true,
        }
    }
}

impl UntrackedMode {
    fn flag(self) -> &'static str {
        match self {
            UntrackedMode::No => "-uno",
            UntrackedMode::Normal => "-unormal",
            UntrackedMode::All => "-uall",
        }
    }
}

/// Collect git info for `cwd`. Returns `None` when not in a repo (or git is
/// unavailable), so callers render a "no git" line.
pub fn collect(cwd: Option<&str>, opts: GitOptions, repo_name: Option<&str>) -> Option<GitInfo> {
    let mut cmd = Command::new("git");
    cmd.args([
        "--no-optional-locks",
        "status",
        "--porcelain=v2",
        "--branch",
        "--show-stash",
        opts.untracked.flag(),
    ]);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let out = run_with_timeout(&mut cmd, GIT_TIMEOUT)?;
    if !out.status.success() {
        return None; // not a repo, or git error
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut info = parse_porcelain_v2(&text)?;
    info.repo_name = repo_name.map(str::to_string);

    // Uncommitted line diff — only when the segment is rendered *and* tracked
    // changes exist, so a clean tree (or a config without the `lines` segment)
    // costs zero extra I/O.
    if opts.lines && info.has_tracked_changes() {
        let (added, removed) = diff_lines(cwd);
        info.diff_added = added;
        info.diff_removed = removed;
    }

    if opts.tags {
        info.tag = describe_tag(cwd);
    }
    Some(info)
}

/// Sum insertions/deletions of uncommitted tracked changes vs HEAD via
/// `git diff --numstat`. Binary files (numstat `-`) are ignored.
fn diff_lines(cwd: Option<&str>) -> (u32, u32) {
    let mut cmd = Command::new("git");
    cmd.args(["--no-optional-locks", "diff", "--numstat", "HEAD"]);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let Some(out) = run_with_timeout(&mut cmd, GIT_TIMEOUT) else {
        return (0, 0);
    };
    if !out.status.success() {
        return (0, 0); // e.g. repo with no commits yet (no HEAD)
    }
    parse_numstat(&String::from_utf8_lossy(&out.stdout))
}

/// Sum the added/removed columns of `git diff --numstat` output. Binary files
/// (columns of `-`) contribute zero.
///
/// Saturating: a generated-file commit can push a diff past `u32::MAX` lines,
/// and a wrapped count (release) or a panic (debug) is worse than a pinned
/// maximum in a status display.
fn parse_numstat(text: &str) -> (u32, u32) {
    let mut added = 0u32;
    let mut removed = 0u32;
    for line in text.lines() {
        let mut cols = line.split('\t');
        let a = cols.next().unwrap_or("-");
        let r = cols.next().unwrap_or("-");
        added = added.saturating_add(a.parse::<u32>().unwrap_or(0));
        removed = removed.saturating_add(r.parse::<u32>().unwrap_or(0));
    }
    (added, removed)
}

fn describe_tag(cwd: Option<&str>) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.args(["describe", "--tags", "--abbrev=0"]);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let out = run_with_timeout(&mut cmd, GIT_TIMEOUT)?;
    if !out.status.success() {
        return None;
    }
    let tag = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if tag.is_empty() { None } else { Some(tag) }
}

/// Parse `git status --porcelain=v2 --branch --show-stash` output.
///
/// Returns `None` only if no branch header is present (not a valid status).
fn parse_porcelain_v2(text: &str) -> Option<GitInfo> {
    let mut info = GitInfo::default();
    let mut saw_branch = false;

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# branch.head ") {
            saw_branch = true;
            if rest == "(detached)" {
                info.detached = true;
            } else {
                info.branch = Some(rest.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("# branch.upstream ") {
            info.upstream = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("# branch.ab ") {
            // "+A -B"
            for tok in rest.split_whitespace() {
                if let Some(n) = tok.strip_prefix('+') {
                    info.ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = tok.strip_prefix('-') {
                    info.behind = n.parse().unwrap_or(0);
                }
            }
        } else if let Some(rest) = line.strip_prefix("# stash ") {
            info.stashes = rest.trim().parse().unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("1 ").or_else(|| line.strip_prefix("2 ")) {
            // Ordinary (1) / renamed-copied (2): "<XY> ..." — XY is the first token.
            if let Some(xy) = rest.split_whitespace().next() {
                tally_xy(xy, &mut info);
            }
        } else if line.starts_with("u ") {
            info.conflicts += 1;
        } else if line.starts_with("? ") {
            info.untracked += 1;
        }
    }

    if saw_branch { Some(info) } else { None }
}

/// Tally a two-char `XY` status code. `X` = staged (index), `Y` = working tree.
fn tally_xy(xy: &str, info: &mut GitInfo) {
    let mut chars = xy.chars();
    let x = chars.next().unwrap_or('.');
    let y = chars.next().unwrap_or('.');

    match x {
        'M' => info.staged_mod += 1,
        'A' => info.staged_add += 1,
        'D' => info.staged_del += 1,
        'R' => info.staged_ren += 1,
        'C' => info.staged_add += 1,
        _ => {}
    }
    match y {
        'M' => info.wt_mod += 1,
        'D' => info.wt_del += 1,
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn parses_clean_branch() {
        let out = "# branch.oid abc\n# branch.head main\n# branch.upstream origin/main\n# branch.ab +0 -0\n";
        let g = parse_porcelain_v2(out).unwrap();
        assert_eq!(g.branch.as_deref(), Some("main"));
        assert_eq!(g.upstream.as_deref(), Some("origin/main"));
        assert_eq!(g.ahead, 0);
        assert_eq!(g.behind, 0);
        assert_eq!(g.total_mod(), 0);
    }

    #[test]
    fn parses_ahead_behind_and_changes() {
        let out = "\
# branch.head feature
# branch.upstream origin/feature
# branch.ab +3 -2
1 M. N... 100644 100644 100644 aaa bbb src/a.rs
1 .M N... 100644 100644 100644 aaa bbb src/b.rs
1 A. N... 000000 100644 100644 000 ccc new.rs
1 D. N... 100644 000000 000000 ddd 000 gone.rs
2 R. N... 100644 100644 100644 eee fff R100 new_name.rs\told_name.rs
? untracked.txt
? another.txt
u UU N... 1 2 3 100644 100644 100644 100644 ggg hhh iii conflict.rs
";
        let g = parse_porcelain_v2(out).unwrap();
        assert_eq!(g.branch.as_deref(), Some("feature"));
        assert_eq!(g.ahead, 3);
        assert_eq!(g.behind, 2);
        assert_eq!(g.staged_mod, 1); // "M." on a.rs
        assert_eq!(g.wt_mod, 1); // ".M" on b.rs
        assert_eq!(g.staged_add, 1); // "A." new.rs
        assert_eq!(g.staged_del, 1); // "D." gone.rs
        assert_eq!(g.staged_ren, 1); // "R." rename
        assert_eq!(g.untracked, 2);
        assert_eq!(g.conflicts, 1);
        assert_eq!(g.total_add(), 3); // 1 staged add + 2 untracked
    }

    #[test]
    fn detached_head() {
        let out = "# branch.head (detached)\n# branch.ab +0 -0\n";
        let g = parse_porcelain_v2(out).unwrap();
        assert!(g.detached);
        assert!(g.branch.is_none());
    }

    #[test]
    fn stash_line() {
        let out = "# branch.head main\n# stash 3\n";
        let g = parse_porcelain_v2(out).unwrap();
        assert_eq!(g.stashes, 3);
    }

    #[test]
    fn not_a_status_returns_none() {
        assert!(parse_porcelain_v2("random text\n").is_none());
    }

    /// BP-001 regression: the timeout path must actually stop the child. The
    /// fixture is Unix-only (`sleep`); the Windows `taskkill` branch is
    /// compile-checked by CI's windows job but has no behavioural fixture here.
    #[cfg(unix)]
    #[test]
    fn terminate_stops_a_running_child() {
        let mut child = Command::new("sleep")
            .arg("30")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let start = Instant::now();
        terminate(child.id());
        let status = child.wait().unwrap();
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "terminate must not wait the sleep out"
        );
        assert!(
            !status.success(),
            "a signalled child must not report success"
        );
    }

    /// BP-017 regression: a diff larger than `u32::MAX` lines must pin at the
    /// maximum, not wrap (release) or panic (debug).
    #[test]
    fn numstat_saturates_instead_of_overflowing() {
        let huge = u32::MAX;
        let out = format!("{huge}\t{huge}\tbig.txt\n{huge}\t{huge}\tbigger.txt\n");
        assert_eq!(parse_numstat(&out), (u32::MAX, u32::MAX));
    }

    #[test]
    fn numstat_sums_and_ignores_binary() {
        let out = "12\t3\tsrc/a.rs\n0\t8\tsrc/b.rs\n-\t-\tlogo.png\n";
        assert_eq!(parse_numstat(out), (12, 11));
    }

    #[test]
    fn has_tracked_changes_excludes_untracked_only() {
        let mut g = GitInfo {
            untracked: 5,
            ..Default::default()
        };
        assert!(!g.has_tracked_changes());
        g.wt_mod = 1;
        assert!(g.has_tracked_changes());
    }

    #[test]
    fn run_with_timeout_kills_hung_process() {
        let mut cmd = Command::new("sleep");
        cmd.arg("5");
        let start = Instant::now();
        let out = run_with_timeout(&mut cmd, Duration::from_millis(100));
        assert!(out.is_none(), "expected timeout to yield None");
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "should return promptly after killing the hung process"
        );
    }

    #[test]
    fn run_with_timeout_returns_output_when_fast() {
        let mut cmd = Command::new("echo");
        cmd.arg("hi");
        let out = run_with_timeout(&mut cmd, Duration::from_secs(2)).unwrap();
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "hi");
    }

    /// Guards the pipe-deadlock the old `try_wait()` poll loop had: it never
    /// read the child's pipes, so any output past the ~64 KiB pipe buffer
    /// wedged the child on `write` until the timeout killed it. 1 MiB of
    /// output must come back whole, well inside the bound.
    #[test]
    fn run_with_timeout_survives_output_larger_than_the_pipe_buffer() {
        let mut cmd = Command::new("sh");
        cmd.args([
            "-c",
            "yes 0123456789012345678901234567890123456789 | head -n 25000",
        ]);
        let start = Instant::now();
        let out = run_with_timeout(&mut cmd, Duration::from_secs(2))
            .expect("large output must not deadlock");
        assert!(out.status.success());
        assert_eq!(out.stdout.len(), 25_000 * 41, "output truncated");
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "should stream through, not hit the timeout"
        );
    }

    /// Build a throwaway repo with one committed file modified in the working
    /// tree (2 lines added, 1 removed vs HEAD). Returns its path.
    fn dirty_fixture(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("statusline-test-{name}"));
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
        std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-qm", "init"]);
        // -1 line ("two"), +2 lines ("2a", "2b")
        std::fs::write(dir.join("a.txt"), "one\n2a\n2b\nthree\n").unwrap();
        dir
    }

    /// A clean tree already skipped the `git diff --numstat` spawn. This pins
    /// the other half: with `segments.lines = false` the diff must be skipped
    /// even on a dirty tree — and the rest of the git state must survive.
    #[test]
    fn lines_disabled_skips_the_diff_but_keeps_git_state() {
        let dir = dirty_fixture("lines-off");
        let cwd = dir.to_str().unwrap();

        let on = collect(
            Some(cwd),
            GitOptions {
                lines: true,
                ..Default::default()
            },
            None,
        )
        .expect("fixture must be a repo");
        assert_eq!(
            (on.diff_added, on.diff_removed),
            (2, 1),
            "baseline: diff is measured when the segment is on"
        );

        let off = collect(
            Some(cwd),
            GitOptions {
                lines: false,
                ..Default::default()
            },
            None,
        )
        .expect("fixture must be a repo");
        assert_eq!(
            (off.diff_added, off.diff_removed),
            (0, 0),
            "diff must be skipped when the lines segment is off"
        );
        assert_eq!(off.wt_mod, 1, "file-level status must still be collected");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
