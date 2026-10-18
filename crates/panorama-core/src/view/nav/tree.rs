//! A thread's agent tree as cards, depth first (Python `render.tree_entries`, `split_group`,
//! `path_ids`): each group of siblings shows its live (or failed) agents, and those with such
//! a descendant; the finished rest fold into one "n finished" card.

use crate::model::{Agent, Thread, TreeKids};
use crate::view::filter::Filter;
use crate::view::state::{FIN_CARD, View};
use std::collections::HashMap;

/// Deeper than this, a tree is cut (a guard; loops are already cut by `TreeKids`).
const MAX_DEPTH: usize = 64;

/// One card of the tree.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeEntry {
    /// The agent's id, or [`FIN_CARD`] + the group key for a "n finished" card.
    pub ident: String,
    /// 1: the thread's own agents.
    pub depth: usize,
    /// The group: the thread's id, or the id of the agent that spawned it.
    pub key: String,
}

/// One group of siblings: the agents shown, and the finished ones folded (Python
/// `split_group`). Shown ones keep their start order (a status change never reorders them);
/// folded ones go most urgent first.
pub fn split_group<'a>(
    ags: &[&'a Agent],
    kids: &TreeKids<'a>,
    filter: Filter,
) -> (Vec<&'a Agent>, Vec<&'a Agent>) {
    let mut memo: HashMap<(bool, &str), bool> = HashMap::new();
    let mut shown = vec![];
    let mut fin = vec![];
    for a in ags {
        if has(a, kids, false, 0, &mut memo) {
            shown.push(*a)
        } else {
            fin.push(*a)
        }
    }
    match filter {
        Filter::Needs => {
            shown.retain(|a| has(a, kids, true, 0, &mut memo));
            fin.clear();
        }
        Filter::Running => fin.clear(),
        Filter::All => {}
    }
    let start = |a: &&Agent| a.started.unwrap_or(0.0);
    shown.sort_by(|a, b| {
        (start(a), a.seq)
            .partial_cmp(&(start(b), b.seq))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    fin.sort_by(|a, b| {
        (a.status.urgency(), start(a))
            .partial_cmp(&(b.status.urgency(), start(b)))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    (shown, fin)
}

/// `a` or one of its descendants is live or failed (`needs`: needs the operator).
fn has<'a>(
    a: &'a Agent,
    kids: &TreeKids<'a>,
    needs: bool,
    depth: usize,
    memo: &mut HashMap<(bool, &'a str), bool>,
) -> bool {
    let key = (needs, a.id.as_str());
    if let Some(&v) = memo.get(&key) {
        return v;
    }
    memo.insert(key, false);
    let me = if needs {
        a.status.needs_operator()
    } else {
        a.status.is_live() || a.status == crate::model::Status::Failed
    };
    let v = me
        || (depth < MAX_DEPTH
            && kids
                .children(Some(&a.id))
                .iter()
                .any(|c| has(c, kids, needs, depth + 1, memo)));
    memo.insert(key, v);
    v
}

/// The thread's tree as cards, depth first.
pub fn tree_entries(t: &Thread, view: &View) -> Vec<TreeEntry> {
    let kids = TreeKids::of(t);
    let mut out = vec![];
    group(&kids, None, &t.id, 1, view, &mut out);
    out
}

fn group(
    kids: &TreeKids,
    pid: Option<&str>,
    key: &str,
    depth: usize,
    view: &View,
    out: &mut Vec<TreeEntry>,
) {
    if depth > MAX_DEPTH {
        return;
    }
    let (shown, fin) = split_group(kids.children(pid), kids, view.filter);
    let card = |a: &Agent, out: &mut Vec<TreeEntry>| {
        out.push(TreeEntry {
            ident: a.id.clone(),
            depth,
            key: key.to_owned(),
        });
        group(kids, Some(&a.id), &a.id, depth + 1, view, out);
    };
    for a in &shown {
        card(a, out);
    }
    if !fin.is_empty() {
        out.push(TreeEntry {
            ident: format!("{FIN_CARD}{key}"),
            depth,
            key: key.to_owned(),
        });
        if view.fin_open.contains(key) {
            for a in &fin {
                card(a, out);
            }
        }
    }
}

/// The selected agent and the agents that spawned it, outermost first.
pub fn path_ids(t: Option<&Thread>, aid: Option<&str>) -> Vec<String> {
    let (Some(t), Some(aid)) = (t, aid) else {
        return vec![];
    };
    let kids = TreeKids::of(t);
    let parent: HashMap<&str, &str> = t
        .agents
        .iter()
        .flat_map(|p| {
            kids.children(Some(&p.id))
                .iter()
                .map(move |c| (c.id.as_str(), p.id.as_str()))
        })
        .collect();
    let mut out: Vec<String> = vec![];
    let mut cur = t.agents.iter().find(|a| a.id == aid).map(|a| a.id.as_str());
    while let Some(c) = cur.filter(|c| !out.iter().any(|o| o == c)) {
        out.push(c.to_owned());
        cur = parent.get(c).copied();
    }
    out.reverse();
    out
}
