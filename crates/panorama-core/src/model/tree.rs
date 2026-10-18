//! The agent tree inside one thread (Python `render.tree_kids`): who spawned whom.

use super::{Agent, Thread};
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
        let ids: HashSet<&str> = thread.agents.iter().map(|a| a.id.as_str()).collect();
        let mut groups: HashMap<Option<&'a str>, Vec<&'a Agent>> = HashMap::new();
        for a in &thread.agents {
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
        let lost: Vec<&Agent> = thread
            .agents
            .iter()
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
}

#[cfg(test)]
mod tests;
