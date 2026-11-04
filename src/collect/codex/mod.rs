//! Neutral Codex observations enrich an already visible herdr pane. Identity and
//! pane runtime status remain herdr's; history supplies the root's title and its child
//! agents, at any depth, as the root's agent tree. The root thread itself is the root: no
//! agent stands for it, and its own state (herdr's status, inbox questions) stays on it.

mod history;
pub(super) mod identity;
mod tree;

use crate::codex::{Observation, Observer, Options};
use crate::collect::agents::Arena;
use crate::collect::tree::Tree;
use panorama_core::model::Root;

/// Only herdr's native Codex identity is authoritative; another harness's
/// session or a custom source cannot become a Codex history lookup key.
pub(super) fn native_id(info: &serde_json::Map<String, serde_json::Value>) -> Option<&str> {
    (info.get("source")?.as_str()? == "herdr:codex"
        && info.get("agent")?.as_str()? == "codex"
        && info
            .get("kind")
            .is_none_or(|kind| kind.as_str() == Some("id")))
    .then(|| info.get("value")?.as_str())
    .flatten()
}

/// What a Codex root's title is chosen from beyond herdr's names: its thread's name and the
/// first prompt (`preview`, else the earliest one read) (plan §2.2 "Root, Codex").
#[derive(Default)]
pub(super) struct RootSeen {
    pub name: Option<String>,
    pub prompt: Option<String>,
}

pub(super) struct Details {
    observer: Observer,
}

impl Details {
    pub(super) fn new(options: Options) -> Self {
        Self {
            observer: Observer::new(options),
        }
    }

    pub(super) fn pending(&self, orchestrator: &Root) -> bool {
        orchestrator.harness == "codex"
            && orchestrator
                .session_id
                .as_deref()
                .is_some_and(|id| self.observer.is_pending(id))
    }

    pub(super) fn enrich(&mut self, orchestrator: &mut Root) -> (RootSeen, Tree) {
        let Some(id) = orchestrator.session_id.clone() else {
            return Default::default();
        };
        let observation = self.observer.observe(&id);
        enrich(orchestrator, observation.as_ref())
    }
}

fn enrich(orchestrator: &mut Root, observation: Option<&Observation>) -> (RootSeen, Tree) {
    orchestrator.last_activity = None;
    let Some(observation) = observation.filter(|value| {
        Some(value.root.thread_id.as_str()) == orchestrator.session_id.as_deref()
            && value.root.parent_thread_id.is_none()
    }) else {
        return Default::default();
    };
    let seen = RootSeen {
        name: observation.root.name.clone(),
        prompt: history::first_prompt(&observation.root).map(str::to_owned),
    };
    let mut arena = Arena::new();
    tree::append(&mut arena, observation);
    orchestrator.last_activity = history::time(observation.root.updated_at);
    // Codex history never proves a child finished, so the done window hides none.
    (seen, Tree::new(arena.into_values(), 0))
}

#[cfg(test)]
mod tests;
