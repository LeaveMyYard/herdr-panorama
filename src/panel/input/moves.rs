//! The cursor (Python `SwarmApp.action_move`, `_resume`, `_select`, `_point`,
//! `action_right`, `action_left`, `_shown`): selection is by identity, column by column.

use crate::panel::model::Panel;
use panorama_core::view::nav::{
    agent_depth, default_children, find_orch, find_thread, nav, parent_key, path_ids,
};
use panorama_core::view::state::FIN_CARD;
use panorama_core::view::{NEW_ROW, View};

impl Panel {
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

    /// ↑/↓ by `delta` in the focused column; from a parked cursor, the next shown row in that
    /// direction. Whether the cursor moved over orchestrators (the arrows' switch then waits
    /// for them to rest).
    pub(crate) fn move_cursor(&mut self, delta: i32) -> bool {
        let col = self.view.focus;
        let cols = nav(&self.swarm, &self.view);
        let Some(items) = cols.get(col).filter(|c| !c.is_empty()) else {
            return false;
        };
        let idx = match self
            .cur(col)
            .and_then(|c| items.iter().position(|i| i == c))
        {
            Some(i) => (i as i64 + i64::from(delta)).clamp(0, items.len() as i64 - 1) as usize,
            None => self.resume(col, items, delta),
        };
        let pick = items[idx].clone();
        self.select(col, &pick);
        col == 0
    }

    /// Where a parked cursor resumes: from the orchestrator it sits on (or was parked off) in
    /// the full list, to the next shown one in the arrow's direction; else the top.
    fn resume(&self, col: usize, items: &[String], delta: i32) -> usize {
        let order: Vec<&str> = self
            .swarm
            .orchestrators
            .iter()
            .map(|o| o.pane_id.as_str())
            .collect();
        let anchor = if col == 0 {
            self.view.orch.as_deref().or(self.parked.as_deref())
        } else {
            None
        };
        let Some(pos) = anchor.and_then(|a| order.iter().position(|p| *p == a)) else {
            return 0;
        };
        // + New agent ranks last; an id not in the list ranks as `missing`
        let rank = |x: &str, missing: i64| -> i64 {
            if x == NEW_ROW {
                order.len() as i64
            } else {
                order
                    .iter()
                    .position(|p| *p == x)
                    .map_or(missing, |i| i as i64)
            }
        };
        let pos = pos as i64;
        if delta > 0 {
            items
                .iter()
                .position(|x| rank(x, -1) > pos)
                .unwrap_or(items.len() - 1)
        } else {
            (0..items.len())
                .rev()
                .find(|&i| rank(&items[i], order.len() as i64) < pos)
                .unwrap_or(0)
        }
    }

    /// →: into the next column (the docked list has one).
    pub(crate) fn right(&mut self) {
        if self.input.dock {
            return;
        }
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
        self.view.focus = nxt;
    }

    /// ←: back a column; from a spawned agent, up to the agent that spawned it.
    pub(crate) fn left(&mut self) {
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
            self.view.focus = focus - 1;
        } else if focus > 0 {
            self.view.focus = focus - 1;
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

#[cfg(test)]
mod tests;
