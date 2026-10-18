//! A card for the orchestrator itself when it needs the operator (Python
//! `collect.main_session_thread`): a blocking prompt (herdr `blocked`), or inbox questions no
//! subagent filed.

use super::Draft;
use crate::collect::agents::Arena;
use herdr_agent_kit::agent_session::claude::Session;
use panorama_core::model::{
    Agent, HerdrStatus, Orchestrator, Phase, PhaseState, Status, ThreadSource, ThreadStatus,
};
use std::collections::HashSet;

/// The card and its one agent, or `None` when the session needs nothing.
pub fn main_session_thread(
    o: &Orchestrator,
    drafts: &[Draft],
    arena: &Arena,
    sess: &Session,
) -> Option<(Draft, Agent)> {
    let attributed: HashSet<i64> = drafts
        .iter()
        .flat_map(|d| &d.ids)
        .flat_map(|id| arena[id].question_ids.iter().copied())
        .collect();
    let loose: Vec<(i64, String)> = o
        .inbox
        .iter()
        .zip(&o.inbox_texts)
        .filter(|(q, _)| !attributed.contains(q))
        .map(|(q, t)| (*q, t.clone()))
        .collect();
    let blocked = o.herdr_status == HerdrStatus::Blocked;
    if !blocked && loose.is_empty() {
        return None;
    }
    let since = sess
        .conversation()
        .prompts()
        .map(|p| p.at.0)
        .reduce(f64::max);
    let id = format!("main:{}", o.pane_id);
    let mut ag = Agent {
        id: id.clone(),
        description: "main session".into(),
        role: "orchestrator".into(),
        status: if blocked {
            Status::Blocked
        } else {
            Status::Waiting
        },
        started: since,
        phases: vec![0],
        ..Agent::default()
    };
    ag.question_ids = loose.iter().map(|(q, _)| *q).collect();
    ag.questions = loose.into_iter().map(|(_, t)| t).collect();
    if blocked {
        ag.activity = "waiting for you in the pane".into();
        ag.pending = o.pending.clone();
    }
    let mut d = Draft::new(id.clone(), o.tab_label.clone(), ThreadSource::Session);
    d.thread.phases = vec![Phase {
        name: "waiting on you".into(),
        state: PhaseState::Now,
        start: since,
        end: None,
        note: None,
    }];
    d.thread.status = ThreadStatus::Active;
    d.thread.started = since;
    d.ids = vec![id];
    Some((d, ag))
}
