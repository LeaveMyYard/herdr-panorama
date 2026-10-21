//! Selection by identity (Python `SwarmApp._cur`, `_select`, `_shown`, `_point`).

use super::Graph;
use panorama_core::view::View;
use panorama_core::view::nav::{
    agent_depth, default_children, find_orch, find_thread, nav, parent_key,
};

impl Graph {
    /// The selection in column `col`: the orchestrator, the thread, or the agent at any depth.
    pub(crate) fn cur(&self, col: usize) -> Option<&str> {
        match col {
            0 => self.view.orch.as_deref(),
            1 => self.view.thread.as_deref(),
            _ => self.view.agent.as_deref(),
        }
    }

    /// Selects `ident` in column `col`, choosing the remembered (else first) children below
    /// it; a selection change scrolls to the selected card again.
    pub(crate) fn select(&mut self, col: usize, ident: &str) {
        let v = &mut self.view;
        v.wheel.clear();
        match col {
            0 => {
                if v.orch.as_deref() != Some(ident) {
                    v.orch = Some(ident.to_owned());
                    default_children(&self.swarm, v);
                }
            }
            1 => {
                if v.thread.as_deref() != Some(ident) {
                    v.thread = Some(ident.to_owned());
                    if let Some(o) = v.orch.clone() {
                        v.remember_thread.insert(o, ident.to_owned());
                    }
                    let cols = nav(&self.swarm, v);
                    let remembered = v.remember_agent.get(ident).cloned();
                    v.agent = remembered
                        .filter(|a| cols[2].contains(a))
                        .or_else(|| cols[2].first().cloned());
                }
            }
            _ => {
                v.agent = Some(ident.to_owned());
                let o = find_orch(&self.swarm, v.orch.as_deref());
                if let Some(t) = find_thread(o, v.thread.as_deref()) {
                    v.remember_agent
                        .insert(parent_key(t, ident), ident.to_owned());
                }
            }
        }
    }

    /// Whether this identity is still selectable (it exists, the filter shows it, it is not
    /// folded away).
    pub(crate) fn shown(&self, pane: &str, thread: Option<&str>, agent: Option<&str>) -> bool {
        let v = View {
            orch: Some(pane.to_owned()),
            thread: thread.map(Into::into),
            agent: agent.map(Into::into),
            ..self.view.clone()
        };
        let cols = nav(&self.swarm, &v);
        cols[0].iter().any(|p| p == pane)
            && thread.is_none_or(|t| cols[1].iter().any(|i| i == t))
            && agent.is_none_or(|a| cols[2..].iter().flatten().any(|i| i == a))
    }

    /// Moves the cursor to an identity, focusing its column.
    pub(crate) fn point(&mut self, pane: &str, thread: Option<&str>, agent: Option<&str>) {
        if self.view.orch.as_deref() != Some(pane) {
            self.select(0, pane);
        }
        if let Some(t) = thread {
            self.select(1, t);
        }
        let col = match (thread, agent) {
            (None, _) => 0,
            (Some(_), None) => 1,
            (Some(t), Some(a)) => {
                let th = find_thread(find_orch(&self.swarm, Some(pane)), Some(t));
                1 + agent_depth(th, Some(a))
            }
        };
        if let Some(a) = agent {
            self.select(col, a);
        }
        self.view.focus = col;
    }
}
