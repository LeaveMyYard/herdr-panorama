//! One `report_thread` call (Python `mcp_server.report_thread`): its arguments checked and
//! cleaned, merged into the thread's declaration, and the result the agent reads.

use crate::collect::events::{MIN_PREFIX, STATUSES};
use crate::collect::paths::events_path;
use crate::report::{MAX_ITEMS, MAX_TEXT, Report, report};
use crate::snapshot::pyjson::J;
use herdr_agent_kit::herdr_api::clean;
use serde_json::{Map, Value};
use std::path::Path;

/// Bidi and format characters a name may not carry.
const INVISIBLE: [char; 12] = [
    '\u{200e}', '\u{200f}', '\u{061c}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}',
    '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
];

/// Control characters cleaned, bidi and format characters stripped, at most 200 characters.
pub fn text(value: &str) -> String {
    clean(value, MAX_TEXT)
        .chars()
        .filter(|c| !INVISIBLE.contains(c))
        .collect()
}

/// An argument that is a string (`None`: absent or null), or why not.
fn string(args: &Map<String, Value>, key: &str) -> Result<Option<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("{key}: a string")),
    }
}

/// An argument that is a list of strings, or why not.
fn strings(args: &Map<String, Value>, key: &str) -> Result<Option<Vec<String>>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or(format!("{key}: a list of strings"))
            })
            .collect::<Result<_, _>>()
            .map(Some),
        Some(_) => Err(format!("{key}: a list of strings")),
    }
}

/// The arguments as the schema says (`Err`: a call the schema refuses).
pub fn arguments(args: &Map<String, Value>) -> Result<Report, String> {
    let thread = string(args, "thread_id")?.ok_or("thread_id: Field required")?;
    Ok(Report {
        thread,
        title: string(args, "title")?,
        phase: string(args, "phase")?,
        phases_done: strings(args, "phases_done")?,
        next: strings(args, "next")?,
        status: string(args, "status")?,
        agents: strings(args, "agents")?,
    })
}

fn answer(status: &str, message: String) -> J {
    J::obj([("status", J::str(status)), ("message", J::Str(message))])
}

/// Why the call is refused, if it is (Python's checks, in its order).
fn refusal(r: &Report) -> Option<String> {
    if r.status.as_deref().is_some_and(|s| !STATUSES.contains(&s)) {
        let mut sorted = STATUSES.to_vec();
        sorted.sort_unstable();
        let list = sorted
            .iter()
            .map(|s| format!("'{s}'"))
            .collect::<Vec<_>>()
            .join(", ");
        return Some(format!("status must be one of [{list}]"));
    }
    for (name, items) in [
        ("phases_done", &r.phases_done),
        ("next", &r.next),
        ("agents", &r.agents),
    ] {
        if items.as_ref().is_some_and(|i| i.len() > MAX_ITEMS) {
            return Some(format!("{name}: at most {MAX_ITEMS} items"));
        }
    }
    let short = r
        .agents
        .iter()
        .flatten()
        .find(|a| a.ends_with('*') && a.trim_end_matches('*').chars().count() < MIN_PREFIX);
    short.map(|a| {
        format!(
            "agent pattern {}: name at least {MIN_PREFIX} characters before *",
            py_repr(a)
        )
    })
}

/// Python's `repr` of a string (quotes as Python picks them).
fn py_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let body: String = s
        .chars()
        .flat_map(|c| match c {
            '\\' => vec!['\\', '\\'],
            '\n' => vec!['\\', 'n'],
            '\t' => vec!['\\', 't'],
            '\r' => vec!['\\', 'r'],
            c if c == quote => vec!['\\', c],
            c => vec![c],
        })
        .collect();
    format!("{quote}{body}{quote}")
}

/// The call for session `sid` (none: nothing recorded), its events in `events_dir`, at `now`.
pub fn report_thread(r: Report, sid: Option<&str>, events_dir: &Path, now: f64) -> J {
    let Some(sid) = sid else {
        return answer(
            "unavailable",
            "No Claude session id; nothing recorded.".into(),
        );
    };
    if let Some(why) = refusal(&r) {
        return answer("error", why);
    }
    let all = |v: Option<Vec<String>>| v.map(|v| v.iter().map(|p| text(p)).collect::<Vec<_>>());
    let cleaned = Report {
        thread: text(&r.thread),
        title: r.title.as_deref().map(text),
        phase: r.phase.as_deref().map(text),
        phases_done: all(r.phases_done),
        next: all(r.next),
        status: r.status,
        agents: all(r.agents).filter(|a| !a.is_empty()),
    };
    match report(&events_path(events_dir, sid), &cleaned, now) {
        Ok(out) => {
            let phases = out
                .phases
                .into_iter()
                .map(|(n, s)| J::Arr(vec![J::Str(n), J::Str(s)]))
                .collect();
            J::obj([
                ("status", J::str("recorded")),
                ("thread", J::Str(out.thread)),
                ("phases", J::Arr(phases)),
                ("thread_status", J::opt_str(out.thread_status)),
            ])
        }
        Err(e) => answer("error", format!("could not record: {e}")),
    }
}

#[cfg(test)]
mod tests;
