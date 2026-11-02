//! Neutral Codex observations enrich an already visible herdr pane. Identity and
//! pane runtime status remain herdr's; history supplies phases and child details.

pub(super) mod channels;
mod declared;
mod history;
mod tree;

use crate::codex::Observation;
use crate::collect::agents::Arena;
use crate::collect::py::chars;
use crate::collect::threads::{Draft, finish};
use channels::Reports;
use panorama_core::model::{Orchestrator, ThreadSource};

pub(super) fn enrich(
    orchestrator: &mut Orchestrator,
    observation: Option<&Observation>,
    reports: &[Reports],
    now: f64,
    window: f64,
) {
    orchestrator.last_activity = None;
    let observation = observation.filter(|value| {
        Some(value.root.thread_id.as_str()) == orchestrator.session_id.as_deref()
            && value.root.parent_thread_id.is_none()
    });
    let root_id = format!("main:codex:{}", orchestrator.pane_id);
    let mut arena = Arena::new();
    if let Some(observation) = observation {
        tree::append(&mut arena, observation, &root_id);
        orchestrator.last_activity = history::time(observation.root.updated_at);
    }
    // Root work is kept in its session card; explicit patterns apply only to
    // child agents. This prevents an optional report from hiding the root card.
    let (mut drafts, claimed) = declared::partition(
        reports,
        orchestrator.session_id.as_deref().unwrap_or(""),
        &mut arena,
        now,
    );
    if let Some(observation) = observation {
        let root = &observation.root;
        let mut agent = history::agent(root, 0);
        agent.id = root_id.clone();
        agent.role = "orchestrator".into();
        agent.depth = 0;
        agent.status = history::runtime(&orchestrator.herdr_status);
        agent.question_ids = orchestrator.inbox.clone();
        agent.questions = orchestrator.inbox_texts.clone();
        agent.pending = orchestrator.pending.clone();
        if root.name.as_deref().is_none_or(str::is_empty) {
            agent.description = chars(&orchestrator.tab_label, 240);
        }
        let mut session = session(orchestrator, observation, &root_id);
        session
            .ids
            .extend(arena.keys().filter(|id| !claimed.contains(*id)).cloned());
        let phases = session.thread.phases.len();
        if phases > 0 {
            agent.phases = vec![phases as i64 - 1];
        }
        arena.insert(root_id, agent);
        drafts.insert(0, session);
    }
    orchestrator.flavour = finish::flavour(&drafts);
    orchestrator.hidden_done = 0;
    orchestrator.threads = drafts
        .into_iter()
        .filter_map(|draft| {
            let thread = draft.into_thread(&arena);
            if thread.is_done() && thread.ended.is_some_and(|end| end < now - window) {
                orchestrator.hidden_done += 1;
                None
            } else {
                Some(thread)
            }
        })
        .collect();
}

fn session(orchestrator: &Orchestrator, observation: &Observation, root_id: &str) -> Draft {
    let mut draft = Draft::new(
        format!("codex:{}", observation.root.thread_id),
        chars(
            observation
                .root
                .name
                .as_deref()
                .filter(|name| !name.is_empty())
                .unwrap_or(&orchestrator.tab_label),
            240,
        ),
        ThreadSource::Session,
    );
    draft.thread.status = history::session_status(&orchestrator.herdr_status);
    draft.thread.started = history::time(observation.root.created_at);
    draft.thread.phases = history::phases(&observation.root, &orchestrator.herdr_status);
    draft.ids.push(root_id.to_owned());
    draft
}

#[cfg(test)]
mod tests;
