//! What can be selected, column by column (Python `render.nav` and its helpers, without the
//! thread column since the uniform agent tree, plan `uniform-agent-tree.md` §4): the roots
//! the filter shows, then the selected root's agent tree, depth by depth. Selection is by
//! identity, never by row.

mod tree;

pub use tree::{TreeEntry, agent_depth, parent_key, path_ids, tree_entries};

use super::filter::visible_orchs;
use super::state::{NEW_ROW, View};
use crate::model::{Agent, Root, Swarm};

/// The root with pane `pane`.
pub fn find_orch<'a>(swarm: &'a Swarm, pane: Option<&str>) -> Option<&'a Root> {
    swarm
        .roots
        .iter()
        .find(|o| Some(o.pane_id.as_str()) == pane)
}

/// `o`'s agent `id`, at any depth.
pub fn find_agent<'a>(o: Option<&'a Root>, id: Option<&str>) -> Option<&'a Agent> {
    o?.agents.iter().find(|a| Some(a.id.as_str()) == id)
}

/// The selectable ids per column, in screen order: the roots (then `+ New agent`), then the
/// selected root's agents by depth (column `n` is depth `n`; there is always a column 1).
pub fn nav(swarm: &Swarm, view: &View) -> Vec<Vec<String>> {
    let orchs = visible_orchs(swarm, view);
    let mut first: Vec<String> = orchs.iter().map(|o| o.pane_id.clone()).collect();
    first.push(NEW_ROW.to_owned());
    let mut cols = vec![first, vec![]];
    let Some(o) = orchs
        .iter()
        .copied()
        .find(|o| Some(&o.pane_id) == view.orch.as_ref())
    else {
        return cols;
    };
    for e in tree_entries(o, view) {
        while cols.len() < 1 + e.depth {
            cols.push(vec![]);
        }
        cols[e.depth].push(e.ident);
    }
    cols
}

/// Selects the remembered (else the first) depth-1 agent below the selected root (Python
/// `SwarmApp._default_children`).
pub fn default_children(swarm: &Swarm, view: &mut View) {
    let cols = nav(swarm, view);
    let remembered = view
        .remember_agent
        .get(view.orch.as_deref().unwrap_or(""))
        .cloned();
    view.agent = remembered
        .filter(|a| cols[1].contains(a))
        .or_else(|| cols[1].first().cloned());
}

#[cfg(test)]
mod tests;
