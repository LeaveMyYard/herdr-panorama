//! A thread declared through the explicit channel (Python `collect.declared_thread`): its
//! phases as declared, and the agents its patterns claim (with the role and phase given).

use super::Draft;
use crate::collect::agents::Arena;
use crate::collect::events::{Declared, STATES, STATUSES, matches, phase_time};
use crate::collect::py::{str_of, truthy_str};
use panorama_core::model::{Phase, PhaseState, ThreadSource, ThreadStatus};
use serde_json::Value;
use std::collections::HashSet;

/// The declared thread; the agents it claims join `claimed`.
pub fn declared_thread(
    d: &Declared,
    arena: &mut Arena,
    claimed: &mut HashSet<String>,
    now: f64,
) -> Draft {
    let phases: Vec<Phase> = d
        .phases
        .iter()
        .map(|p| Phase {
            name: str_of(p, "name").unwrap_or("").to_owned(),
            state: PhaseState::from_code(
                str_of(p, "state")
                    .filter(|s| STATES.contains(s))
                    .unwrap_or("next"),
            ),
            start: phase_time(p.get("start"), d.ts, now),
            end: phase_time(p.get("end"), d.ts, now),
            note: truthy_str(p, "note"),
        })
        .collect();
    let mut ids = Vec::new();
    for spec in &d.agents {
        let pattern = str_of(spec, "match").unwrap_or("");
        let candidates: Vec<String> = arena.keys().cloned().collect();
        for id in candidates {
            if claimed.contains(&id) || !matches(pattern, &id, &arena[&id].description) {
                continue;
            }
            claimed.insert(id.clone());
            let a = &mut arena[&id];
            if let Some(role) = str_of(spec, "role") {
                a.role = role.into();
            }
            let phase = spec
                .get("phase")
                .filter(|v| !v.is_boolean())
                .and_then(Value::as_i64);
            if let Some(ph) = phase.filter(|ph| (0..phases.len() as i64).contains(ph)) {
                a.phases = vec![ph];
            }
            ids.push(id);
        }
    }
    let status = d
        .status
        .as_deref()
        .filter(|s| STATUSES.contains(s))
        .unwrap_or("");
    let mut t = Draft::new(
        format!("x:{}", d.thread),
        d.name.clone().unwrap_or_else(|| d.thread.clone()),
        ThreadSource::Explicit,
    );
    t.thread.phases = phases;
    t.thread.status = ThreadStatus::from_code(status);
    t.thread.worktree = d.worktree.clone();
    t.thread.pct = d.pct;
    t.thread.flavour = d.flavour.clone();
    t.ids = ids;
    t
}
