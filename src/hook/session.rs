//! SessionStart (Python `hooks.session_start`, `prune`): the session's process recorded and
//! old hook files pruned. Nothing is said to Claude Code: the `report_thread` instruction was
//! removed with threads (plan `uniform-agent-tree.md` §6), so the hook's stdout stays empty.

use super::Ctx;
use super::record::{VERSION, append};
use crate::snapshot::pyjson::J;
use serde_json::{Map, Value};
use std::path::Path;
use std::time::{Duration, SystemTime};

/// Hook files older than this are pruned.
pub const PRUNE_AFTER: Duration = Duration::from_secs(14 * 86_400);
/// At most this many directory entries are looked at per prune.
const PRUNE_SCAN: usize = 500;

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

/// Record the session. Says nothing: the record's `told` stays in the v1 format, always false.
pub fn session_start(payload: &Map<String, Value>, cx: &Ctx) {
    if cx.host.host == crate::host::Host::Core
        && (payload.get("hook_event_name").and_then(Value::as_str) != Some("SessionStart")
            || payload.get("agent_id").is_some_and(|v| !v.is_null())
            || payload
                .get("parent_session_id")
                .is_some_and(|v| !v.is_null()))
    {
        return;
    }
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs_f64(cx.now.max(0.0));
    prune(&cx.hooks_dir, now);
    let Some(session) = payload
        .get("session_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    else {
        return;
    };
    let fields = vec![
        ("v".into(), J::Int(VERSION)),
        ("event".into(), J::str("session")),
        ("agent".into(), J::str("")),
        (
            "source".into(),
            payload.get("source").map_or(J::Null, J::from),
        ),
        ("ts".into(), J::Float(cx.now)),
        ("told".into(), J::Bool(false)),
    ];
    // a record that cannot be written is dropped silently, as every hook drops it
    let _ = append(session, fields, cx);
}
