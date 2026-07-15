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
use std::process::Command;

/// Options controlling the git query (wired to config in Phase 4).
#[derive(Debug, Clone, Copy)]
pub struct GitOptions {
    /// `"normal"` (default), `"no"` (skip untracked scan — fastest), `"all"`.
    pub untracked: UntrackedMode,
    /// Also run `git describe` for the latest tag (extra spawn).
    pub tags: bool,
}

// `No`/`All` are selected via config in Phase 4; `Normal` is the default today.
#[allow(dead_code)]
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
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None; // not a repo, or git error
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut info = parse_porcelain_v2(&text)?;
    info.repo_name = repo_name.map(str::to_string);

    // Uncommitted line diff — only when tracked changes exist, so a clean tree
    // costs zero extra I/O (nothing to diff, resets naturally after a commit).
    if info.has_tracked_changes() {
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
    let Ok(out) = cmd.output() else {
        return (0, 0);
    };
    if !out.status.success() {
        return (0, 0); // e.g. repo with no commits yet (no HEAD)
    }
    parse_numstat(&String::from_utf8_lossy(&out.stdout))
}

/// Sum the added/removed columns of `git diff --numstat` output. Binary files
/// (columns of `-`) contribute zero.
fn parse_numstat(text: &str) -> (u32, u32) {
    let mut added = 0u32;
    let mut removed = 0u32;
    for line in text.lines() {
        let mut cols = line.split('\t');
        let a = cols.next().unwrap_or("-");
        let r = cols.next().unwrap_or("-");
        added += a.parse::<u32>().unwrap_or(0);
        removed += r.parse::<u32>().unwrap_or(0);
    }
    (added, removed)
}

fn describe_tag(cwd: Option<&str>) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.args(["describe", "--tags", "--abbrev=0"]);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let out = cmd.output().ok()?;
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
}
