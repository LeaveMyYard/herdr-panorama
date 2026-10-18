//! The explicit thread channel, read side (Python `events.Channel`, `matches`,
//! `phase_time`): one append-only JSONL file per session, each line declaring or updating one
//! thread field by field (`phases` and `agents` replace). Followed incrementally; a file that
//! shrank or was replaced (a compaction) is read again from the start.

use super::py::{or_time, str_of};
use chrono::{Local, TimeZone};
use herdr_agent_kit::agent_session::claude::wire::{self, Line};
use herdr_agent_kit::agent_session::tail::{Tail, Tailer};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// The channel's format version.
pub const VERSION: f64 = 1.0;
/// An agent pattern `abc*` names at least this much before the `*`.
pub const MIN_PREFIX: usize = 4;
/// A phase's states.
pub const STATES: [&str; 5] = ["done", "now", "next", "failed", "skipped"];
/// A thread's statuses.
pub const STATUSES: [&str; 4] = ["active", "done", "failed", "cancelled"];

/// One thread as the lines so far declare it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Declared {
    pub thread: String,
    pub name: Option<String>,
    pub flavour: Option<String>,
    pub status: Option<String>,
    pub worktree: Option<String>,
    pub pct: Option<i64>,
    pub phases: Vec<Map<String, Value>>,
    pub agents: Vec<Map<String, Value>>,
    /// The latest line's time.
    pub ts: Option<f64>,
}

/// One session's events file, followed.
#[derive(Debug)]
pub struct Channel {
    path: PathBuf,
    tail: Tailer,
    /// The declared threads, in the order they were first declared.
    pub threads: Vec<Declared>,
}

/// Python's `rec.get("v") == 1` (`True` and `1.0` equal it too).
fn version_ok(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Bool(b)) => *b,
        Some(v) => v.as_f64() == Some(VERSION),
        None => false,
    }
}

impl Channel {
    /// The channel of the file at `path`, nothing read yet.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            tail: Tailer::new(),
            threads: Vec::new(),
        }
    }

    /// Read what was appended since the last update.
    pub fn update(&mut self) {
        let mut read = self.tail.read(&self.path);
        if read == Tail::Reset {
            self.threads.clear();
            read = self.tail.read(&self.path);
        }
        if let Tail::Lines(lines) = read {
            for raw in lines {
                if let Some(rec) = Line::parse(&raw) {
                    self.apply(rec.map());
                }
            }
        }
    }

    /// Merge one line into its thread.
    pub fn apply(&mut self, rec: &Map<String, Value>) {
        let Some(tid) = str_of(rec, "thread").filter(|t| !t.is_empty()) else {
            return;
        };
        if !version_ok(rec.get("v")) {
            return;
        }
        let i = match self.threads.iter().position(|d| d.thread == tid) {
            Some(i) => i,
            None => {
                self.threads.push(Declared {
                    thread: tid.into(),
                    ..Declared::default()
                });
                self.threads.len() - 1
            }
        };
        let d = &mut self.threads[i];
        let text = |k: &str| str_of(rec, k).map(str::to_owned);
        d.name = text("name").or(d.name.take());
        d.flavour = text("flavour").or(d.flavour.take());
        d.worktree = text("worktree").or(d.worktree.take());
        if let Some(s) = str_of(rec, "status").filter(|s| STATUSES.contains(s)) {
            d.status = Some(s.into());
        }
        if let Some(p) = rec
            .get("pct")
            .filter(|p| p.is_number())
            .and_then(Value::as_f64)
            .filter(|p| p.is_finite())
        {
            d.pct = Some((p.trunc() as i64).clamp(0, 100));
        }
        let objects = |key: &str, field: &str| {
            rec.get(key).and_then(Value::as_array).map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_object)
                    .filter(|o| o.get(field).is_some_and(Value::is_string))
                    .cloned()
                    .collect::<Vec<_>>()
            })
        };
        if let Some(phases) = objects("phases", "name") {
            d.phases = phases;
        }
        if let Some(agents) = objects("agents", "match") {
            d.agents = agents;
        }
        d.ts = or_time(rec.get("ts").and_then(wire::ts).map(|t| t.secs()), d.ts);
    }

    /// The file this channel follows.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Whether an agent pattern claims an agent: its id, its exact description, or `prefix*`
/// (at least [`MIN_PREFIX`] characters) on its description.
pub fn matches(pattern: &str, agent_id: &str, description: &str) -> bool {
    if pattern == agent_id {
        return true;
    }
    if pattern.ends_with('*') {
        let prefix = pattern.trim_end_matches('*');
        return prefix.chars().count() >= MIN_PREFIX && description.starts_with(prefix);
    }
    description == pattern
}

/// `HH:MM` (local time, on the reference day: `reference`, else `now`) or ISO-8601, to
/// POSIX seconds. Hours and minutes out of range roll over, as `mktime` does.
pub fn phase_time(value: Option<&Value>, reference: Option<f64>, now: f64) -> Option<f64> {
    let value = value.and_then(Value::as_str).filter(|v| !v.is_empty())?;
    if value.chars().count() <= 5 && value.contains(':') {
        let parts: Vec<&str> = value.split(':').collect();
        let [hh, mm] = parts[..] else { return None };
        let (hh, mm): (i64, i64) = (hh.trim().parse().ok()?, mm.trim().parse().ok()?);
        let base = or_time(reference, Some(now)).unwrap_or(now);
        let day = Local
            .timestamp_opt(base.floor() as i64, 0)
            .earliest()?
            .date_naive();
        let at = day.and_hms_opt(0, 0, 0)? + chrono::Duration::seconds(hh * 3600 + mm * 60);
        return Local
            .from_local_datetime(&at)
            .earliest()
            .map(|t| t.timestamp() as f64);
    }
    wire::ts(&Value::String(value.into())).map(|t| t.secs())
}

#[cfg(test)]
mod tests;
