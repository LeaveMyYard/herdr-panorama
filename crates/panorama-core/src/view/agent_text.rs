//! How an agent is named on screen (Python `render.role_label`, `short_id`, `age_of`).

use super::text::dur;
use crate::model::Agent;

/// The role, else the agent type, else "agent".
pub fn role_label(a: &Agent) -> &str {
    [a.role.as_str(), a.agent_type.as_deref().unwrap_or("")]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or("agent")
}

/// What a view calls it: its title, else (a producer that predates titles) `role id`.
pub fn name(a: &Agent) -> String {
    if a.title.is_empty() {
        format!("{} {}", role_label(a), short_id(a))
    } else {
        a.title.clone()
    }
}

/// Its secondary line under a title: `role id · model`; `None` when it has no title (the
/// role and id are then its name).
pub fn about(a: &Agent) -> Option<String> {
    if a.title.is_empty() {
        return None;
    }
    let model = a
        .model
        .as_deref()
        .filter(|m| !m.is_empty())
        .map_or(String::new(), |m| format!(" · {m}"));
    Some(format!("{} {}{model}", role_label(a), short_id(a)))
}

/// The id's first 7 characters.
pub fn short_id(a: &Agent) -> String {
    a.id.chars().take(7).collect()
}

/// How long it ran (still running: until `now`); `None` without a start time.
pub fn age_of(a: &Agent, now: f64) -> Option<String> {
    let started = a.started.filter(|s| *s != 0.0)?;
    let end = if a.status.is_live() {
        now
    } else {
        a.ended.filter(|e| *e != 0.0).unwrap_or(now)
    };
    Some(dur(Some(end - started)))
}
