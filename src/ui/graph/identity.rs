//! Selection by identity (Python `SwarmApp._cur`, `_select`, `_shown`, `_point`).

use super::Graph;
use panorama_core::view::View;
use panorama_core::view::nav::{agent_depth, default_children, find_orch, nav, parent_key};

impl Graph {
    /// The selection in column `col`: the root, or the agent at any depth.
    pub(crate) fn cur(&self, col: usize) -> Option<&str> {
        match col {
            0 => self.view.orch.as_deref(),
            _ => self.view.agent.as_deref(),
        }
    }

    /// Selects `ident` in column `col`, choosing the remembered (else first) children below
    /// it; a selection change scrolls to the selected card again.
    pub(crate) fn select(&mut self, col: usize, ident: &str) {
        let v = &mut self.view;
        v.wheel.clear(); // a selection change scrolls to the selected card again
        v.hcol = None; // and sideways to the focus
        match col {
            0 => {
                if v.orch.as_deref() != Some(ident) {
                    v.orch = Some(ident.to_owned());
                    default_children(&self.swarm, v);
                }
            }
            _ => {
                v.agent = Some(ident.to_owned());
                if let Some(o) = find_orch(&self.swarm, v.orch.as_deref()) {
                    v.remember_agent
                        .insert(parent_key(o, ident), ident.to_owned());
                }
            }
        }
    }

    /// Whether this identity is still selectable (it exists, the filter shows it, it is not
    /// folded away).
    pub(crate) fn shown(&self, pane: &str, agent: Option<&str>) -> bool {
        let v = View {
            orch: Some(pane.to_owned()),
            agent: agent.map(Into::into),
            ..self.view.clone()
        };
        let cols = nav(&self.swarm, &v);
        cols[0].iter().any(|p| p == pane)
            && agent.is_none_or(|a| cols[1..].iter().flatten().any(|i| i == a))
    }

    /// Moves the cursor to an identity, focusing its column (its depth).
    pub(crate) fn point(&mut self, pane: &str, agent: Option<&str>) {
        if self.view.orch.as_deref() != Some(pane) {
            self.select(0, pane);
        }
        let col = match agent {
            None => 0,
            Some(a) => agent_depth(find_orch(&self.swarm, Some(pane)), Some(a)),
        };
        if let Some(a) = agent {
            self.select(col, a);
        }
        self.focus(col);
    }
}
