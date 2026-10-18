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

/// "main" for the session's own work, else the id's first 7 characters.
pub fn short_id(a: &Agent) -> String {
    if a.id.starts_with("main:") {
        "main".into()
    } else {
        a.id.chars().take(7).collect()
    }
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
