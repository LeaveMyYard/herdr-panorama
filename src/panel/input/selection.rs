//! When the action keys act, and what they report seen (Python `SwarmApp.armed`, `_seen`,
//! `PanelApp._usable`); what they act on is the graph's `selection`.

use super::effect::{Effect, Toast};
use crate::panel::model::Panel;
use crate::panel::paint::NOTICE;
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
            self.graph.view.unseen.remove(id);
        }
        if !ids.is_empty() {
            out.push(Effect::Seen(ids));
        }
    }

}

#[cfg(test)]
mod tests;
