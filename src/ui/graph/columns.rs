//! →/← through the columns (Python `action_right`, `action_left`): into the agents the
//! selected one spawned, back up to the one that spawned it; the docked list has one.

use super::Graph;
use panorama_core::view::nav::{find_orch, find_thread, nav, parent_key, path_ids};
use panorama_core::view::state::FIN_CARD;

impl Graph {
    /// →: into the next column (a view's `right` guards the docked list's one column).
    pub(crate) fn step_right(&mut self) {
        let cols = nav(&self.swarm, &self.view);
        let nxt = self.view.focus + 1;
        if cols.get(nxt).is_none_or(|c| c.is_empty()) {
            return;
        }
        let o = find_orch(&self.swarm, self.view.orch.as_deref());
        let t = find_thread(o, self.view.thread.as_deref());
        let agent = self.view.agent.clone();
        if self.view.focus >= 2 {
            // into the agents the selected one spawned (the one last selected there, else the first)
            let (Some(t), Some(a)) = (t, agent.as_deref()) else {
                return;
            };
            if a.starts_with(FIN_CARD) {
                return;
            }
            let kids: Vec<&String> = cols[nxt].iter().filter(|i| parent_key(t, i) == a).collect();
            let Some(first) = kids.first() else { return };
            let remembered = self
                .view
                .remember_agent
                .get(a)
                .filter(|r| kids.contains(r))
                .cloned();
            let pick = remembered.unwrap_or_else(|| (*first).clone());
            self.select(nxt, &pick);
        } else if nxt == 2 && agent.as_ref().is_some_and(|a| !cols[2].contains(a)) {
            // a deeper agent stays selected: step into the depth-1 agent on its path
            let path = path_ids(t, agent.as_deref());
            let pick = path
                .first()
                .filter(|p| cols[2].contains(p))
                .unwrap_or(&cols[2][0])
                .clone();
            self.select(2, &pick);
        } else if self
            .cur(nxt)
            .is_none_or(|c| !cols[nxt].iter().any(|i| i == c))
        {
            let pick = cols[nxt][0].clone();
            self.select(nxt, &pick);
        }
        self.focus(nxt);
    }

    /// ←: back a column; from a spawned agent, up to the agent that spawned it.
    pub(crate) fn step_left(&mut self) {
        let focus = self.view.focus;
        if focus >= 3 {
            let o = find_orch(&self.swarm, self.view.orch.as_deref());
            if let (Some(t), Some(a)) = (
                find_thread(o, self.view.thread.as_deref()),
                self.view.agent.clone(),
            ) {
                let key = parent_key(t, &a);
                if key != t.id {
                    self.select(focus - 1, &key);
                }
            }
            self.focus(focus - 1);
        } else if focus > 0 {
            self.focus(focus - 1);
        }
    }
}
