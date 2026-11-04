//! The agent tree inside one thread (Python `render.tree_kids`): who spawned whom.

use super::{Agent, Orchestrator, Thread};
use std::collections::{HashMap, HashSet};

/// A thread's agents grouped by the agent that spawned them, in the thread's order.
///
/// The thread's own agents are the group of `None`. An agent whose parent is not in the
/// thread, or is itself, or that only a parent loop reaches, counts as the thread's own, so
/// no agent is ever hidden.
#[derive(Debug, Default)]
pub struct TreeKids<'a> {
    /// The thread's own agents.
    own: Vec<&'a Agent>,
    /// The others, by the id of the agent that spawned them.
    spawned: HashMap<&'a str, Vec<&'a Agent>>,
}

impl<'a> TreeKids<'a> {
    /// The tree of `thread`'s agents.
    pub fn of(thread: &'a Thread) -> Self {
        Self::from_agents(&thread.agents)
    }

    /// The tree of a root's agents across all its threads (plan `uniform-agent-tree.md`
    /// §2.1): its own agents are the root's direct children.
    pub fn of_root(o: &'a Orchestrator) -> Self {
        Self::from_agents(o.agents())
    }

    /// The tree of `agents`, in their order.
    pub fn from_agents(agents: impl IntoIterator<Item = &'a Agent>) -> Self {
        let agents: Vec<&'a Agent> = agents.into_iter().collect();
        let ids: HashSet<&str> = agents.iter().map(|a| a.id.as_str()).collect();
        let mut groups: HashMap<Option<&'a str>, Vec<&'a Agent>> = HashMap::new();
        for &a in &agents {
            let parent = a
                .parent
                .as_deref()
                .filter(|p| ids.contains(p) && *p != a.id);
            groups.entry(parent).or_default().push(a);
        }
        // Walk from the thread's own agents: whatever the walk misses sits in a loop.
        let mut seen: HashSet<&str> = HashSet::new();
        let mut stack: Vec<Option<&str>> = vec![None];
        while let Some(p) = stack.pop() {
            for a in groups.get(&p).into_iter().flatten() {
                if seen.insert(a.id.as_str()) {
                    stack.push(Some(a.id.as_str()));
                }
            }
        }
        let lost: Vec<&Agent> = agents
            .iter()
            .copied()
            .filter(|a| !seen.contains(a.id.as_str()))
            .collect();
        if !lost.is_empty() {
            for group in groups.values_mut() {
                group.retain(|a| seen.contains(a.id.as_str()));
            }
        }
        let mut own = groups.remove(&None).unwrap_or_default();
        own.extend(lost);
        let spawned = groups
            .into_iter()
            .filter_map(|(p, kids)| Some((p?, kids)))
            .collect();
        Self { own, spawned }
    }

    /// The agents `parent` spawned (`None`: the thread's own), in the thread's order.
    pub fn children(&self, parent: Option<&str>) -> &[&'a Agent] {
        match parent {
            None => &self.own,
            Some(p) => self.spawned.get(p).map_or(&[], Vec::as_slice),
        }
    }

    /// Whether `parent` spawned any agent.
    pub fn has_children(&self, parent: &str) -> bool {
        !self.children(Some(parent)).is_empty()
    }

    /// Everything below agent `id`, depth first (cut at [`MAX_DEPTH`] levels, a guard: loops
    /// are already cut).
    pub fn descendants(&self, id: &str) -> Vec<&'a Agent> {
        let mut out = vec![];
        self.walk(id, 0, &mut out);
        out
    }

    /// `a` and everything below it.
    pub fn subtree(&self, a: &'a Agent) -> Vec<&'a Agent> {
        let mut out = vec![a];
        self.walk(&a.id, 0, &mut out);
        out
    }

    fn walk(&self, id: &str, depth: usize, out: &mut Vec<&'a Agent>) {
        if depth > MAX_DEPTH {
            return;
        }
        for k in self.children(Some(id)) {
            out.push(k);
            self.walk(&k.id, depth + 1, out);
        }
    }
}

/// How deep [`TreeKids::descendants`] walks.
pub const MAX_DEPTH: usize = 64;

#[cfg(test)]
mod tests;
