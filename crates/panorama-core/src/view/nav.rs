//! What can be selected, column by column (Python `render.nav` and its helpers): the
//! orchestrators the filter shows, the selected one's threads, and the selected thread's
//! agent tree, depth by depth. Selection is by identity, never by row.

mod tree;

pub use tree::{TreeEntry, agent_depth, parent_key, path_ids, tree_entries};

use super::filter::{Filter, visible_orchs};
use super::state::{BG_CARD, DONE_CARD, NEW_ROW, View};
use crate::model::{Agent, Orchestrator, Swarm, Thread, ThreadSource, ThreadStatus};

/// An orchestrator's threads as the graph shows them: the active ones (a failed thread stays
/// with them: it needs the operator), and the done ones it folds (Python `split_threads`).
pub fn split_threads(o: &Orchestrator, filter: Filter) -> (Vec<&Thread>, Vec<&Thread>) {
    let is_active = |t: &Thread| {
        (!t.is_done() && t.status != ThreadStatus::Failed) || t.source == ThreadSource::Session
    };
    let mut active: Vec<&Thread> = o.threads.iter().filter(|t| is_active(t)).collect();
    let rest = o.threads.iter().filter(|t| !is_active(t));
    let (failed, done): (Vec<&Thread>, Vec<&Thread>) =
        rest.partition(|t| t.status == ThreadStatus::Failed);
    active.extend(failed);
    match filter {
        Filter::Needs => {
            let needs = |t: &&Thread| {
                t.agents.iter().any(|a| a.status.needs_operator())
                    || t.status == ThreadStatus::Failed
            };
            (active.into_iter().filter(needs).collect(), vec![])
        }
        Filter::Running => (
            active
                .into_iter()
                .filter(|t| t.status == ThreadStatus::Active)
                .collect(),
            vec![],
        ),
        Filter::All => (active, done),
    }
}

/// The orchestrator with pane `pane`.
pub fn find_orch<'a>(swarm: &'a Swarm, pane: Option<&str>) -> Option<&'a Orchestrator> {
    swarm
        .orchestrators
        .iter()
        .find(|o| Some(o.pane_id.as_str()) == pane)
}

/// `o`'s thread `id`.
pub fn find_thread<'a>(o: Option<&'a Orchestrator>, id: Option<&str>) -> Option<&'a Thread> {
    o?.threads.iter().find(|t| Some(t.id.as_str()) == id)
}

/// `t`'s agent `id`.
pub fn find_agent<'a>(t: Option<&'a Thread>, id: Option<&str>) -> Option<&'a Agent> {
    t?.agents.iter().find(|a| Some(a.id.as_str()) == id)
}

/// The selectable ids per column, in screen order: orchestrators (then `+ New agent`), the
/// selected one's threads (its background card first, its "n complete" card after the
/// active ones, the done ones when unfolded), then the selected thread's agents by depth.
pub fn nav(swarm: &Swarm, view: &View) -> Vec<Vec<String>> {
    let orchs = visible_orchs(swarm, view);
    let mut first: Vec<String> = orchs.iter().map(|o| o.pane_id.clone()).collect();
    first.push(NEW_ROW.to_owned());
    let mut cols = vec![first, vec![], vec![]];
    let Some(o) = orchs
        .iter()
        .copied()
        .find(|o| Some(&o.pane_id) == view.orch.as_ref())
    else {
        return cols;
    };
    let (active, done) = split_threads(o, view.filter);
    if !o.bg.is_empty() {
        cols[1].push(format!("{BG_CARD}{}", o.pane_id));
    }
    cols[1].extend(active.iter().map(|t| t.id.clone()));
    if !done.is_empty() {
        cols[1].push(format!("{DONE_CARD}{}", o.pane_id));
        if view.done_open.contains(&o.pane_id) {
            cols[1].extend(done.iter().map(|t| t.id.clone()));
        }
    }
    if let Some(t) = find_thread(Some(o), view.thread.as_deref()) {
        for e in tree_entries(t, view) {
            while cols.len() < 2 + e.depth {
                cols.push(vec![]);
            }
            cols[1 + e.depth].push(e.ident);
        }
    }
    cols
}

/// Selects the remembered (else the first) thread and agent below the selected orchestrator
/// (Python `SwarmApp._default_children`).
pub fn default_children(swarm: &Swarm, view: &mut View) {
    let cols = nav(swarm, view);
    let remembered = view
        .remember_thread
        .get(view.orch.as_deref().unwrap_or(""))
        .cloned();
    view.thread = remembered
        .filter(|t| cols[1].contains(t))
        .or_else(|| cols[1].first().cloned());
    let cols = nav(swarm, view);
    let remembered = view
        .remember_agent
        .get(view.thread.as_deref().unwrap_or(""))
        .cloned();
    view.agent = remembered
        .filter(|a| cols[2].contains(a))
        .or_else(|| cols[2].first().cloned());
}

#[cfg(test)]
mod tests;
