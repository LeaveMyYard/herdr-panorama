//! What counts as an unseen event (Python `unseen.Tracker._event`, `urgent`, the record
//! update): a new failure, a new question, a finished agent. The most urgent mark wins.

use panorama_core::model::Status;
use serde_json::{Map, Value, json};

/// An unseen mark, most urgent first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Event {
    Fail,
    Wait,
    Done,
}

impl Event {
    /// The state file's word for it.
    pub fn code(self) -> &'static str {
        match self {
            Event::Fail => "fail",
            Event::Wait => "wait",
            Event::Done => "done",
        }
    }

    /// From the state file (anything else: no mark).
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "fail" => Some(Event::Fail),
            "wait" => Some(Event::Wait),
            "done" => Some(Event::Done),
            _ => None,
        }
    }
}

/// The more urgent of two marks.
pub fn urgent(a: Option<Event>, b: Option<Event>) -> Option<Event> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

fn finished(code: &str) -> bool {
    code == "done" || code == "inf"
}

/// The event between the recorded state (`prev_status`, its question ids) and the agent's now.
pub fn event(
    prev_status: Option<&str>,
    prev_qs: &[i64],
    status: &Status,
    question_ids: &[i64],
) -> Option<Event> {
    let mut ev = None;
    if *status == Status::Failed && prev_status != Some("fail") {
        ev = urgent(ev, Some(Event::Fail));
    }
    if question_ids.iter().any(|q| !prev_qs.contains(q)) {
        ev = urgent(ev, Some(Event::Wait));
    }
    let was_finished = prev_status.is_some_and(finished) || prev_status == Some("cxl");
    if finished(status.code()) && !was_finished {
        ev = urgent(ev, Some(Event::Done));
    }
    ev
}

/// A record's question ids (numbers only).
pub fn qids(rec: &Map<String, Value>) -> Vec<i64> {
    rec.get("q")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_i64)
        .collect()
}

/// A record's mark.
pub fn mark(rec: &Map<String, Value>) -> Option<Event> {
    rec.get("ev")
        .and_then(Value::as_str)
        .and_then(Event::from_code)
}

/// A fresh record of an agent seen now.
pub fn record(status: &Status, question_ids: &[i64], now: f64) -> Map<String, Value> {
    let r = json!({"s": status.code(), "q": question_ids, "ev": null, "t": now});
    r.as_object().cloned().unwrap_or_default()
}

/// A JSON number or a numeric string as Python's `float(x or 0)` reads it.
pub fn float(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        Some(Value::String(s)) if !s.is_empty() => s.trim().parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests;
