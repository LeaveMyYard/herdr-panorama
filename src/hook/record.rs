//! A hook record (Python `hooks.record`, `_append`): one JSON line appended to
//! `<hooks dir>/<session>.jsonl` with one write (`O_APPEND` keeps lines whole when hooks run
//! at once), with the Claude process's identity when it is recognised (else no pid is written
//! and the scanner never calls an agent lost).

use super::Ctx;
use crate::collect::paths::hooks_path;
use crate::snapshot::pyjson::{J, dumps_line_ascii};
use serde_json::{Map, Value};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;

/// The record format's version.
pub const VERSION: i64 = 1;

/// The docs show both `agent-abc123` and `def456`; files are `agent-<id>.jsonl`.
pub fn normalize_agent_id(value: &str) -> &str {
    value.strip_prefix("agent-").unwrap_or(value)
}

/// Append `fields` (and the process identity) to the session's hook file.
pub fn append(session: &str, mut fields: Vec<(String, J)>, cx: &Ctx) -> io::Result<()> {
    if let Some((pid, start, pidns)) = (cx.identity)() {
        fields.push(("pid".into(), J::Int(i64::from(pid))));
        fields.push(("pstart".into(), J::Str(start)));
        fields.push(("pidns".into(), J::opt_str(pidns)));
    }
    let path = hooks_path(&cx.hooks_dir, session);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let line = dumps_line_ascii(&J::Obj(fields)) + "\n";
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o644)
        .open(&path)?;
    file.write_all(line.as_bytes())
}

fn text<'a>(payload: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

/// A subagent's start or stop. Nothing without a session and an agent, or when Claude Code
/// is already continuing because of a stop hook (`stop_hook_active`).
pub fn record(event: &str, payload: &Map<String, Value>, cx: &Ctx) {
    let (Some(session), Some(agent)) = (text(payload, "session_id"), text(payload, "agent_id"))
    else {
        return;
    };
    if payload.get("stop_hook_active") == Some(&Value::Bool(true)) {
        return;
    }
    let agent_type = payload
        .get("agent_type")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let fields = vec![
        ("v".into(), J::Int(VERSION)),
        ("event".into(), J::str(event)),
        ("agent".into(), J::str(normalize_agent_id(agent))),
        ("agent_type".into(), J::opt_str(agent_type)),
        ("ts".into(), J::Float(cx.now)),
    ];
    let _ = append(session, fields, cx);
}
