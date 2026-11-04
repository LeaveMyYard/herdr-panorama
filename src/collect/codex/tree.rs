//! Exact-ID child hierarchy. Disconnected, duplicate or cyclic observations do
//! not become plausible-looking root agents.

use super::history;
use crate::codex::{Observation, Thread};
use crate::collect::agents::Arena;
use std::collections::{HashMap, HashSet};

/// Insert descendants only when their parent chain reaches this exact root; the root's own
/// children have no parent (they are the root's).
pub(super) fn append(arena: &mut Arena, observation: &Observation) {
    let root = observation.root.thread_id.as_str();
    for (depth, thread) in descendants(observation) {
        let mut agent = history::agent(thread, arena.len() as i64 + 1);
        agent.depth = depth as i64;
        agent.parent = thread
            .parent_thread_id
            .as_deref()
            .filter(|parent| *parent != root)
            .map(str::to_owned);
        arena.insert(agent.id.clone(), agent);
    }
}

pub(super) fn descendants(observation: &Observation) -> Vec<(usize, &Thread)> {
    let root = observation.root.thread_id.as_str();
    let mut seen = HashSet::new();
    let mut duplicate = HashSet::new();
    for thread in &observation.children {
        if !seen.insert(thread.thread_id.as_str()) {
            duplicate.insert(thread.thread_id.as_str());
        }
    }
    let by_id: HashMap<&str, &Thread> = observation
        .children
        .iter()
        .filter(|thread| {
            !thread.thread_id.is_empty()
                && thread.thread_id != root
                && !duplicate.contains(thread.thread_id.as_str())
        })
        .map(|thread| (thread.thread_id.as_str(), thread))
        .collect();
    let mut children = Vec::new();
    for thread in &observation.children {
        if !by_id.contains_key(thread.thread_id.as_str()) {
            continue;
        }
        if let Some(depth) = depth(thread, root, &by_id) {
            children.push((depth, thread));
        }
    }
    children.sort_by(|(a_depth, a), (b_depth, b)| {
        a_depth
            .cmp(b_depth)
            .then(a.created_at.total_cmp(&b.created_at))
            .then(a.thread_id.cmp(&b.thread_id))
    });
    children
}

fn depth(thread: &Thread, root: &str, by_id: &HashMap<&str, &Thread>) -> Option<usize> {
    let mut visited = HashSet::from([thread.thread_id.as_str()]);
    let mut parent = thread.parent_thread_id.as_deref()?;
    let mut depth = 1;
    while parent != root {
        if !visited.insert(parent) {
            return None;
        }
        parent = by_id.get(parent)?.parent_thread_id.as_deref()?;
        depth += 1;
    }
    Some(depth)
}
