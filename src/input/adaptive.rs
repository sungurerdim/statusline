//! Adaptive, harness-agnostic input parser.
//!
//! Instead of binding to one harness's exact schema, we walk the incoming JSON
//! as a generic tree and pull each concept (model, cwd, cost, context, rate
//! limits, effort) from *any* of several known field paths. Whatever a harness
//! provides is used; whatever it omits is simply absent. No configuration, no
//! per-harness branch — the same binary adapts to Claude Code, Cursor CLI,
//! OpenCode's proposed shape, Codex's, or a bare generic object automatically.
//!
//! Adding support for a new harness = adding a candidate path to a list here.

use crate::model::{RateWindow, StatusData};
use serde_json::Value;

/// Follow a dotted path of object keys, returning the value if every hop exists.
fn at<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = v;
    for key in path {
        cur = cur.get(key)?;
    }
    Some(cur)
}

/// First path that resolves to a string.
fn str_at(v: &Value, paths: &[&[&str]]) -> Option<String> {
    paths
        .iter()
        .find_map(|p| at(v, p).and_then(Value::as_str))
        .map(str::to_string)
}

/// First path that resolves to an unsigned integer (accepts JSON floats too).
fn u64_at(v: &Value, paths: &[&[&str]]) -> Option<u64> {
    paths.iter().find_map(|p| {
        let n = at(v, p)?;
        n.as_u64().or_else(|| n.as_f64().map(|f| f as u64))
    })
}

/// First path that resolves to a float.
fn f64_at(v: &Value, paths: &[&[&str]]) -> Option<f64> {
    paths.iter().find_map(|p| at(v, p).and_then(Value::as_f64))
}

fn short_model(name: &str) -> String {
    name.strip_prefix("Claude ").unwrap_or(name).to_string()
}

fn basename(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    trimmed.rsplit('/').next().map(str::to_string)
}

/// Sum the input-side tokens of a `current_usage`-style object (Claude Code's
/// input-only context accounting), from whichever path holds it.
fn context_tokens_from_usage(v: &Value) -> Option<u64> {
    let usage = at(v, &["context_window", "current_usage"])
        .or_else(|| at(v, &["current_usage"]))
        .or_else(|| at(v, &["usage"]))?;
    let field = |k: &str| usage.get(k).and_then(Value::as_u64).unwrap_or(0);
    let total = field("input_tokens")
        + field("cache_creation_input_tokens")
        + field("cache_read_input_tokens");
    (total > 0).then_some(total)
}

/// Extract a rate-limit window (`used_percentage` + `resets_at`) by name.
fn window(v: &Value, key: &str) -> Option<RateWindow> {
    let w = at(v, &["rate_limits", key]).or_else(|| at(v, &[key]))?;
    let used_pct = w
        .get("used_percentage")
        .or_else(|| w.get("used_pct"))
        .and_then(Value::as_f64)?;
    let resets_at = w
        .get("resets_at")
        .or_else(|| w.get("reset_at"))
        .and_then(Value::as_i64);
    Some(RateWindow {
        used_pct,
        resets_at,
    })
}

/// Build normalized [`StatusData`] from any harness's JSON payload.
pub fn extract(v: &Value) -> StatusData {
    let cwd = str_at(
        v,
        &[
            &["workspace", "current_dir"],
            &["cwd"],
            &["workspace", "cwd"],
            &["directory"],
        ],
    );

    let project_name = str_at(
        v,
        &[
            &["workspace", "git_worktree"],
            &["workspace", "repo", "name"],
            &["repo", "name"],
        ],
    )
    .or_else(|| cwd.as_deref().and_then(basename));

    let model_name = str_at(
        v,
        &[
            &["model", "display_name"],
            &["model", "name"],
            &["model", "id"],
        ],
    )
    .or_else(|| v.get("model").and_then(Value::as_str).map(str::to_string))
    .map(|s| short_model(&s));

    let context_tokens = context_tokens_from_usage(v).or_else(|| {
        u64_at(
            v,
            &[
                &["context_window", "total_input_tokens"],
                &["tokens", "used"],
                &["context", "used"],
            ],
        )
    });
    let context_size = u64_at(
        v,
        &[
            &["context_window", "context_window_size"],
            &["tokens", "total"],
            &["context", "size"],
        ],
    );
    let context_used_pct = f64_at(
        v,
        &[
            &["context_window", "used_percentage"],
            &["tokens", "used_percentage"],
            &["context", "used_percentage"],
        ],
    )
    .or_else(|| match (context_tokens, context_size) {
        (Some(t), Some(s)) if s > 0 => Some(t as f64 * 100.0 / s as f64),
        _ => None,
    });

    StatusData {
        cwd,
        project_name,
        model_name,
        effort_level: str_at(v, &[&["effort", "level"], &["reasoning", "effort"]]),
        context_tokens,
        context_used_pct,
        cost_usd: f64_at(
            v,
            &[
                &["cost", "total_cost_usd"],
                &["cost", "total_usd"],
                &["cost_usd"],
                &["session", "cost", "total_usd"],
            ],
        ),
        duration_ms: u64_at(v, &[&["cost", "total_duration_ms"], &["duration_ms"]]),
        api_duration_ms: u64_at(v, &[&["cost", "total_api_duration_ms"]]),
        five_hour: window(v, "five_hour"),
        seven_day: window(v, "seven_day"),
        git: None,
    }
}

/// Parse a JSON string into [`StatusData`]; malformed input yields defaults so
/// the tool always renders (git-only, etc.) rather than erroring.
pub fn parse(json: &str) -> StatusData {
    match serde_json::from_str::<Value>(json) {
        Ok(v) => extract(&v),
        Err(_) => StatusData::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLAUDE_CODE: &str = r#"{
      "cwd": "/home/u/proj",
      "model": { "id": "claude-opus-4-8", "display_name": "Claude Opus 4.8" },
      "workspace": { "current_dir": "/home/u/proj", "repo": { "name": "proj" } },
      "version": "2.1.90",
      "effort": { "level": "high" },
      "cost": { "total_cost_usd": 0.42, "total_duration_ms": 4980000, "total_api_duration_ms": 2000000 },
      "context_window": {
        "context_window_size": 1000000,
        "used_percentage": 17,
        "current_usage": { "input_tokens": 8500, "cache_creation_input_tokens": 5000, "cache_read_input_tokens": 156500 }
      },
      "rate_limits": { "five_hour": { "used_percentage": 23.5, "resets_at": 1738425600 }, "seven_day": { "used_percentage": 2 } }
    }"#;

    // A different, hypothetical harness shape (OpenCode-style proposal): flat
    // tokens object, model.name, cost.total_usd, workspace.cwd.
    const OTHER_HARNESS: &str = r#"{
      "model": { "name": "Sonnet 5" },
      "workspace": { "cwd": "/work/api-server" },
      "tokens": { "used": 50000, "total": 200000 },
      "cost": { "total_usd": 1.5 }
    }"#;

    #[test]
    fn parses_claude_code_shape() {
        let d = parse(CLAUDE_CODE);
        assert_eq!(d.model_name.as_deref(), Some("Opus 4.8"));
        assert_eq!(d.project_name.as_deref(), Some("proj"));
        assert_eq!(d.context_tokens, Some(170000));
        assert_eq!(d.context_used_pct, Some(17.0));
        assert_eq!(d.cost_usd, Some(0.42));
        assert_eq!(d.api_duration_ms, Some(2000000));
        assert_eq!(d.five_hour.as_ref().unwrap().used_pct, 23.5);
        assert_eq!(d.seven_day.as_ref().unwrap().used_pct, 2.0);
    }

    #[test]
    fn adapts_to_a_different_harness_shape() {
        let d = parse(OTHER_HARNESS);
        assert_eq!(d.model_name.as_deref(), Some("Sonnet 5"));
        assert_eq!(d.project_name.as_deref(), Some("api-server"));
        // context derived from flat tokens.used / tokens.total
        assert_eq!(d.context_tokens, Some(50000));
        assert_eq!(d.context_used_pct, Some(25.0));
        assert_eq!(d.cost_usd, Some(1.5));
        assert!(d.five_hour.is_none()); // not provided → omitted, not wrong
    }

    #[test]
    fn empty_and_invalid_are_safe() {
        assert!(parse("{}").model_name.is_none());
        assert!(parse("not json").model_name.is_none());
    }
}
