//! SessionStart (Python `hooks.session_start`, `_told`, `prune`): the session's process
//! recorded, old hook files pruned, and the `report_thread` line printed the first time a
//! session starts inside herdr.

use super::Ctx;
use super::record::{VERSION, append};
use crate::collect::paths::hooks_path;
use crate::snapshot::pyjson::{J, dumps_line_ascii};
use serde_json::{Map, Value};
use std::path::Path;
use std::time::{Duration, SystemTime};

/// Hook files older than this are pruned.
pub const PRUNE_AFTER: Duration = Duration::from_secs(14 * 86_400);
/// At most this many directory entries are looked at per prune.
const PRUNE_SCAN: usize = 500;

/// The line Claude Code adds to the session's context.
pub const SESSION_TEXT: &str = "herdr-panorama shows your work in a swarm view. When you run multi-step work (a PR through review rounds, a plan's tasks, several subagents on one goal), call report_thread when a thread starts and whenever its phase changes: thread_id, title, the current phase, finished phases, next phases. One call per change; skip it for single-step tasks.";

/// Whether the session was told already (a session record with `"told": true`).
fn told(path: &Path) -> bool {
    std::fs::read(path).is_ok_and(|bytes| {
        bytes.split(|b| *b == b'\n').any(|l| {
            let has = |needle: &[u8]| l.windows(needle.len()).any(|w| w == needle);
            has(b"\"event\": \"session\"") && has(b"\"told\": true")
        })
    })
}

/// Delete hook files not written for [`PRUNE_AFTER`] (the first 500 entries of the dir).
pub fn prune(dir: &Path, now: SystemTime) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let Some(cutoff) = now.checked_sub(PRUNE_AFTER) else {
        return;
    };
    for e in entries.take(PRUNE_SCAN).flatten() {
        let old = std::fs::metadata(e.path())
            .and_then(|m| m.modified())
            .is_ok_and(|t| t < cutoff);
        if e.file_name().to_string_lossy().ends_with(".jsonl") && old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// Record the session; the output for Claude Code, if any.
pub fn session_start(payload: &Map<String, Value>, cx: &Ctx) -> Option<String> {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs_f64(cx.now.max(0.0));
    prune(&cx.hooks_dir, now);
    let session = payload
        .get("session_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())?;
    let tell = cx.in_herdr && !told(&hooks_path(&cx.hooks_dir, session));
    let fields = vec![
        ("v".into(), J::Int(VERSION)),
        ("event".into(), J::str("session")),
        ("agent".into(), J::str("")),
        (
            "source".into(),
            payload.get("source").map_or(J::Null, J::from),
        ),
        ("ts".into(), J::Float(cx.now)),
        ("told".into(), J::Bool(tell)),
    ];
    // a session record that cannot be written: nothing is said (Python raises before printing)
    append(session, fields, cx).ok()?;
    let out = J::obj([(
        "hookSpecificOutput",
        J::obj([
            ("hookEventName", J::str("SessionStart")),
            ("additionalContext", J::str(SESSION_TEXT)),
        ]),
    )]);
    tell.then(|| dumps_line_ascii(&out))
}
