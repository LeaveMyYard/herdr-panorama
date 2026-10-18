//! What the action keys act on, and when (Python `SwarmApp.armed`, `_selection`, `_seen`,
//! `PanelApp._usable`).

use super::effect::{Effect, Toast};
use crate::panel::model::Panel;
use crate::panel::paint::NOTICE;
use panorama_core::model::{Agent, Orchestrator, Thread};
use panorama_core::view::NEW_ROW;
use panorama_core::view::nav::{find_agent, find_orch, find_thread};
use std::time::Instant;

impl Panel {
    /// Keys act: nothing changed under them in the last [`super::ARM`].
    pub(crate) fn armed(&self, now: Instant) -> bool {
        now >= self.input.armed_at
    }

    /// Without a core the panel acts on nothing that needs one, and says so.
    pub(crate) fn usable(&self, out: &mut Vec<Effect>) -> bool {
        if !self.connected {
            out.push(Effect::Toast(Toast::new(None, NOTICE, 3)));
        }
        self.connected
    }

    /// The operator looked at these agents (the actor, after a confirmed switch): their marks
    /// clear here, and the core hears.
    pub fn seen_now(&mut self, ids: Vec<String>) -> Vec<Effect> {
        let mut out = vec![];
        self.seen(ids, &mut out);
        out
    }

    /// The operator looked at these agents: their marks clear here at once, and the core hears.
    pub(crate) fn seen(&mut self, ids: Vec<String>, out: &mut Vec<Effect>) {
        for id in &ids {
            self.view.unseen.remove(id);
        }
        if !ids.is_empty() {
            out.push(Effect::Seen(ids));
        }
    }

    /// What the action keys act on: (orchestrator, thread, agent); nothing while the focused
    /// column's cursor is parked, or on + New agent.
    pub(crate) fn selection(&self) -> (Option<&Orchestrator>, Option<&Thread>, Option<&Agent>) {
        if self.cur(self.view.focus).is_none() || self.view.orch.as_deref() == Some(NEW_ROW) {
            return (None, None, None);
        }
        let o = find_orch(&self.swarm, self.view.orch.as_deref());
        let t = find_thread(o, self.view.thread.as_deref());
        let a = if self.view.focus >= 2 || t.is_some() {
            find_agent(t, self.view.agent.as_deref())
        } else {
            None
        };
        (o, t, a)
    }
}

#[cfg(test)]
mod tests;
