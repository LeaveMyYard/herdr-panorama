//! The conflict rule and cross-terminal uniqueness (session-linking.md §4).
use super::Signal;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
    Linked(String),
    Unavailable,
    Conflict,
}

/// Every present signal must name the same id; ambiguity or disagreement refuses. A
/// `Newest` hook signal counts only when another signal names that same id (§4.2).
pub fn combine(signals: &[Signal]) -> Link {
    if signals.contains(&Signal::Ambiguous) {
        return Link::Unavailable;
    }
    let named: Vec<&String> = signals
        .iter()
        .filter_map(|s| match s {
            Signal::Names(id) => Some(id),
            _ => None,
        })
        .collect();
    let newest: Vec<&String> = signals
        .iter()
        .filter_map(|s| match s {
            Signal::Newest(id) => Some(id),
            _ => None,
        })
        .collect();
    let Some(first) = named.first() else {
        return Link::Unavailable;
    };
    if named.iter().chain(&newest).any(|id| id != first) {
        return Link::Conflict;
    }
    Link::Linked((*first).clone())
}

/// A session or foreground PID claimed by more than one terminal refuses every claimant.
pub fn unique(links: &mut HashMap<String, (Option<u32>, Link)>) {
    let mut sessions: HashMap<String, usize> = HashMap::new();
    let mut pids: HashMap<u32, usize> = HashMap::new();
    for (pid, link) in links.values() {
        if let Some(pid) = pid {
            *pids.entry(*pid).or_default() += 1;
        }
        if let Link::Linked(id) = link {
            *sessions.entry(id.clone()).or_default() += 1;
        }
    }
    for (pid, link) in links.values_mut() {
        let shared_pid = pid.is_some_and(|p| pids[&p] > 1);
        let shared_id = matches!(link, Link::Linked(id) if sessions[id] > 1);
        if shared_pid || shared_id {
            *link = Link::Unavailable;
        }
    }
}
