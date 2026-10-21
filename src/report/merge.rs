//! An agent's report merged into its thread's declaration (Python `events.report`).
//!
//! Phases keep their times across calls: a phase gets `start` when it is first reported as
//! current (or done), and `end` when it is first reported as done. Omitted lists keep what was
//! declared before; `phase` moves to the front of the phases not done.

use super::emit::emit;
use super::stamp;
use crate::collect::events::{Channel, Declared, STATUSES};
use crate::snapshot::pyjson::J;
use serde_json::{Map, Value};
use std::io;
use std::path::Path;

/// One `report_thread` call (its texts already cleaned).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub thread: String,
    pub title: Option<String>,
    pub phase: Option<String>,
    pub phases_done: Option<Vec<String>>,
    pub next: Option<Vec<String>>,
    pub status: Option<String>,
    pub agents: Option<Vec<String>>,
}

/// What was recorded: the thread, its phases (name, state) and the status written, if any.
#[derive(Clone, Debug, PartialEq)]
pub struct Reported {
    pub thread: String,
    pub phases: Vec<(String, String)>,
    pub thread_status: Option<String>,
}

/// The thread's declaration as the file says now.
pub fn current(path: &Path, thread: &str) -> Option<Declared> {
    let mut ch = Channel::new(path.to_path_buf());
    ch.update();
    ch.threads.into_iter().find(|d| d.thread == thread)
}

fn state(p: &Map<String, Value>) -> Option<&str> {
    p.get("state").and_then(Value::as_str)
}

/// The phases of the merged declaration.
fn phases(r: &Report, prev: Option<&Declared>, now: &str) -> Vec<Map<String, Value>> {
    let mut old: Vec<(String, Map<String, Value>)> = Vec::new();
    for p in prev.map_or(&[][..], |d| &d.phases) {
        let name = p
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        match old.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = p.clone(),
            None => old.push((name, p.clone())),
        }
    }
    let get = |n: &str| old.iter().find(|(k, _)| k == n).map(|(_, p)| p);
    let phase = r.phase.as_deref().filter(|p| !p.is_empty());
    let named = |st: &str| {
        old.iter()
            .filter(|(_, p)| state(p) == Some(st))
            .map(|(n, _)| n.clone())
            .collect::<Vec<_>>()
    };
    let mut done = r.phases_done.clone().unwrap_or_else(|| named("done"));
    if let Some(ph) = phase {
        done.retain(|n| n != ph);
    }
    let next: Vec<String> = r
        .next
        .clone()
        .unwrap_or_else(|| named("next"))
        .into_iter()
        .filter(|n| Some(n.as_str()) != phase && !done.contains(n))
        .collect();
    let fresh = |n: &str| {
        get(n)
            .cloned()
            .unwrap_or_else(|| Map::from_iter([("name".to_owned(), Value::from(n))]))
    };
    let mut out = Vec::new();
    for n in &done {
        let was_done = get(n).and_then(state) == Some("done");
        let mut p = fresh(n);
        p.insert("state".into(), "done".into());
        if !p.contains_key("start") {
            let start = p
                .get("end")
                .filter(|e| crate::collect::py::truthy(e))
                .cloned()
                .unwrap_or_else(|| now.into());
            p.insert("start".into(), start);
        }
        if !was_done || !p.contains_key("end") {
            p.insert("end".into(), now.into());
        }
        out.push(p);
    }
    if let Some(ph) = phase {
        let mut p = fresh(ph);
        let st = match r.status.as_deref() {
            Some("failed") => "failed",
            Some("done") => "done",
            _ => "now",
        };
        p.insert("state".into(), st.into());
        p.entry("start").or_insert_with(|| now.into());
        if st == "done" {
            p.insert("end".into(), now.into());
        }
        out.push(p);
    }
    out.extend(next.iter().map(|n| {
        Map::from_iter([
            ("name".to_owned(), n.as_str().into()),
            ("state".to_owned(), "next".into()),
        ])
    }));
    out
}

/// Merge `r` into the thread's declaration in the events file at `path` and append it, at
/// `now`.
pub fn report(path: &Path, r: &Report, now: f64) -> io::Result<Reported> {
    let prev = current(path, &r.thread);
    let phases = phases(r, prev.as_ref(), &stamp(now));
    let mut rec = vec![
        ("thread".to_owned(), J::str(&r.thread)),
        (
            "phases".to_owned(),
            J::Arr(
                phases
                    .iter()
                    .map(|p| J::from(&Value::Object(p.clone())))
                    .collect(),
            ),
        ),
    ];
    if let Some(t) = r.title.as_deref().filter(|t| !t.is_empty()) {
        rec.push(("name".into(), J::str(t)));
    }
    let status = match r.status.as_deref() {
        Some(s) if STATUSES.contains(&s) => Some(s.to_owned()),
        _ if prev.as_ref().is_none_or(|d| d.status.is_none()) => Some("active".to_owned()),
        _ => None,
    };
    if let Some(s) = &status {
        rec.push(("status".into(), J::str(s)));
    }
    if let Some(agents) = r.agents.as_ref().filter(|a| !a.is_empty()) {
        let specs = agents
            .iter()
            .map(|a| J::obj([("match", J::str(a))]))
            .collect();
        rec.push(("agents".into(), J::Arr(specs)));
    }
    emit(path, rec, now)?;
    let names = phases.iter().map(|p| {
        let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        (s("name"), s("state"))
    });
    Ok(Reported {
        thread: r.thread.clone(),
        phases: names.collect(),
        thread_status: status,
    })
}

#[cfg(test)]
mod tests;
