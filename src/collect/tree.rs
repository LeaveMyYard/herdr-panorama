//! A root's agents as one tree (plan `uniform-agent-tree.md` §2.1, slice U3).
//!
//! The tree is flat, parents before children. `parent: None` is a direct child of the root;
//! a parent that is not an agent of the same root, or that closes a cycle, makes the agent a
//! root child, so no agent is ever hidden. `depth` is the length of the parent chain (root
//! children are 1). Siblings come most urgent first, then oldest first.

use panorama_core::model::{Agent, Root};
use std::collections::{HashMap, HashSet};

/// A root's agents and how many finished ones the done window left out.
#[derive(Debug, Default)]
pub struct Tree {
    pub agents: Vec<Agent>,
    pub hidden: i64,
}

impl Tree {
    /// The tree of these agents, in any order.
    pub fn new(agents: impl IntoIterator<Item = Agent>, hidden: i64) -> Self {
        Self {
            agents: ordered(agents.into_iter().collect()),
            hidden,
        }
    }
}

fn sibling_order(a: &Agent, b: &Agent) -> std::cmp::Ordering {
    a.status.urgency().cmp(&b.status.urgency()).then(
        a.started
            .unwrap_or(0.0)
            .total_cmp(&b.started.unwrap_or(0.0)),
    )
}

/// Parents before children, each agent's parent and depth set as the module says.
fn ordered(mut all: Vec<Agent>) -> Vec<Agent> {
    all.sort_by(sibling_order);
    let ids: HashMap<String, usize> = all
        .iter()
        .enumerate()
        .map(|(i, a)| (a.id.clone(), i))
        .collect();
    let parent: Vec<Option<usize>> = all
        .iter()
        .enumerate()
        .map(|(i, a)| {
            a.parent
                .as_ref()
                .and_then(|p| ids.get(p).copied())
                .filter(|&p| p != i)
        })
        .collect();
    let mut kids: Vec<Vec<usize>> = vec![Vec::new(); all.len()];
    for (i, p) in parent.iter().enumerate() {
        if let Some(p) = p {
            kids[*p].push(i);
        }
    }
    let mut order: Vec<(usize, Option<String>, i64)> = Vec::with_capacity(all.len());
    let mut placed = vec![false; all.len()];
    let roots: Vec<usize> = (0..all.len()).filter(|&i| parent[i].is_none()).collect();
    // whatever is left after the root children hangs off a cycle: walking up from it, the
    // agent whose parent closes the cycle becomes a root child
    for start in roots.into_iter().chain(0..all.len()) {
        if placed[start] {
            continue;
        }
        let mut top = start;
        let mut seen = HashSet::from([start]);
        while let Some(p) = parent[top] {
            if !seen.insert(p) {
                break;
            }
            top = p;
        }
        let mut stack = vec![(top, None::<String>, 1_i64)];
        while let Some((i, up, depth)) = stack.pop() {
            if std::mem::replace(&mut placed[i], true) {
                continue;
            }
            order.push((i, up, depth));
            let id = Some(all[i].id.clone());
            for &k in kids[i].iter().rev() {
                stack.push((k, id.clone(), depth + 1));
            }
        }
    }
    let mut slots: Vec<Option<Agent>> = all.into_iter().map(Some).collect();
    order
        .into_iter()
        .map(|(i, parent, depth)| {
            let mut a = slots[i].take().expect("each agent placed once");
            (a.parent, a.depth) = (parent, depth);
            a
        })
        .collect()
}

/// Put the tree on the root: its agents and its count of hidden ones.
pub fn plant(o: &mut Root, tree: Tree) {
    o.agents = tree.agents;
    o.hidden_finished = tree.hidden;
}

#[cfg(test)]
mod tests;
