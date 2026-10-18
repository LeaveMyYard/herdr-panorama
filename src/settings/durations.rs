//! The two duration settings in panorama's `config.toml` (Python `config.parse_duration`,
//! `done_window`, `stale_after`): how long finished threads stay visible, and how long an
//! idle orchestrator waits before it reads "stale". Each is seconds or a string such as
//! `"90m"`, `"1h"`, `"2h30m"`; an environment variable overrides the file.
//!
//! Deviation, stated: Python's `isdigit` and `\d` also take non-ASCII digits; here only ASCII
//! digits count, so such a value takes the default.

use serde_json::{Map, Value};

/// `done_window`'s default: one hour.
pub const DEFAULT_DONE_WINDOW: f64 = 3600.0;
/// `stale_after`'s default: twelve hours.
pub const DEFAULT_STALE_AFTER: f64 = 12.0 * 3600.0;

/// A duration in seconds: a number (not negative, not a bool), digits, or `<h>h<m>m<s>s` with
/// each part optional (at least one) and spaces between them allowed, in any case.
pub fn parse_duration(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64().filter(|v| *v >= 0.0),
        Value::String(s) => parse_text(s),
        _ => None,
    }
}

/// [`parse_duration`] of a string (an environment variable's value).
pub fn parse_text(s: &str) -> Option<f64> {
    let t = s.trim();
    if !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()) {
        return t.parse().ok();
    }
    let lower = s.to_lowercase();
    let mut rest = lower.as_str();
    let mut total = 0.0;
    let mut any = false;
    for (unit, secs) in [('h', 3600.0), ('m', 60.0), ('s', 1.0)] {
        rest = rest.trim_start();
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        if digits == 0 {
            continue;
        }
        let after = rest[digits..].trim_start();
        if let Some(tail) = after.strip_prefix(unit) {
            total += rest[..digits].parse::<f64>().ok()? * secs;
            any = true;
            rest = tail;
        }
    }
    (any && rest.trim().is_empty()).then_some(total)
}

fn setting(env: Option<&str>, cfg: &Map<String, Value>, key: &str, default: f64) -> f64 {
    if let Some(v) = env.filter(|v| !v.is_empty()).and_then(parse_text) {
        return v;
    }
    cfg.get(key).and_then(parse_duration).unwrap_or(default)
}

/// `done_window`: `$PANORAMA_DONE_WINDOW` (`env`), else the file's key, else one hour.
pub fn done_window(env: Option<&str>, cfg: &Map<String, Value>) -> f64 {
    setting(env, cfg, "done_window", DEFAULT_DONE_WINDOW)
}

/// `stale_after`: `$PANORAMA_STALE_AFTER` (`env`), else the file's key, else twelve hours.
pub fn stale_after(env: Option<&str>, cfg: &Map<String, Value>) -> f64 {
    setting(env, cfg, "stale_after", DEFAULT_STALE_AFTER)
}

#[cfg(test)]
mod tests;
