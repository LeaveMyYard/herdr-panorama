//! The graph a view shows and its cursor (Python `SwarmApp`'s model and selection, which
//! `PanelApp` inherits): the swarm, the view (what is selected, filtered, scrolled) and where
//! a parked cursor resumes. The panel and the app each hold one; what differs between them
//! (the docked list, the arming, the herdr side) stays theirs.
//!
//! - [`identity`]: selection by identity (`select`, `point`, `shown`);
//! - [`vertical`]: ↑/↓ and resuming a parked cursor; [`columns`]: →/←;
//! - [`pointer`]: the mouse's hover and wheel.

mod columns;
mod identity;
mod pointer;
mod vertical;

use crate::ui::selection;
use panorama_core::model::{Agent, Orchestrator, Swarm, Thread};
use panorama_core::view::nav::{find_agent, find_orch, find_thread, tree_entries};
use panorama_core::view::state::FIN_CARD;
use panorama_core::view::{NEW_ROW, View};

/// A view's graph: its swarm, its view state, and where a parked cursor resumes.
#[derive(Debug, Default)]
pub struct Graph {
    /// The last swarm (an empty one before the first).
    pub swarm: Swarm,
    /// What is drawn: the selection, the filter, the unseen marks, the scroll.
    pub view: View,
    /// Where the arrows resume from while no orchestrator is selected.
    pub(crate) parked: Option<String>,
}

impl Graph {
    /// The selectable rows: the orchestrators the filter shows, then `+ New agent`.
    pub fn shown_ids(&self) -> Vec<String> {
        selection::shown_ids(&self.swarm, &self.view)
    }

    /// After a refresh or a filter change: every selection that still exists stays, an
    /// orchestrator no longer shown is parked (Python `SwarmApp.resolve`).
    pub(crate) fn resolve(&mut self) {
        if let Some(p) = selection::resolve(&self.swarm, &mut self.view) {
            self.parked = Some(p);
        }
    }

    /// Where the arrows resume from while no orchestrator is selected.
    pub fn parked(&self) -> Option<&str> {
        self.parked.as_deref()
    }

    /// What the action keys act on: (orchestrator, thread, agent); nothing while the focused
    /// column's cursor is parked, or on + New agent (Python `_selection`).
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

    /// The agents folded into a group's "n finished" card (Python `_fin_agents`).
    pub(crate) fn fin_agents(&self, key: &str) -> Vec<String> {
        let o = find_orch(&self.swarm, self.view.orch.as_deref());
        let Some(t) = find_thread(o, self.view.thread.as_deref()) else {
            return vec![];
        };
        let card = format!("{FIN_CARD}{key}");
        tree_entries(t, &self.view)
            .into_iter()
            .find(|e| e.ident == card)
            .map_or(vec![], |e| e.fin)
    }
}

/// Adds `key` to `set`, or takes it out.
pub(crate) fn toggle(set: &mut std::collections::BTreeSet<String>, key: &str) {
    if !set.remove(key) {
        set.insert(key.to_owned());
    }
}

#[cfg(test)]
mod tests;
