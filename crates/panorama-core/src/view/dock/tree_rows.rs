//! A docked entry's agent tree (plan `uniform-agent-tree.md` §4, V11 sidebar): who spawned
//! whom across the whole root, and which agents are still going: live, or with a live
//! descendant. The root's direct children are layer 1, theirs layer 2.

use crate::model::tree::MAX_DEPTH;
use crate::model::{Agent, Root, Status, TreeKids};
use crate::view::anim::glyph;
use std::collections::HashSet;

/// One root's agents as a tree, with the ids of those still going.
#[derive(Debug)]
pub struct Tree<'a> {
    kids: TreeKids<'a>,
    busy: HashSet<&'a str>,
}

impl<'a> Tree<'a> {
    /// The tree of `o`'s agents.
    pub fn of(o: &'a Root) -> Self {
        let kids = TreeKids::of(o);
        let mut busy = HashSet::new();
        for a in kids.children(None) {
            mark_busy(a, &kids, 0, &mut busy);
        }
        Self { kids, busy }
    }

    /// Live, or with a live descendant.
    pub fn is_busy(&self, a: &Agent) -> bool {
        self.busy.contains(a.id.as_str())
    }

    /// The agents `parent` spawned (`None`: the root's own), by start time then `seq` (a
    /// status change never reorders them): those still going, and the finished ones.
    pub fn split(&self, parent: Option<&str>) -> (Vec<&'a Agent>, Vec<&'a Agent>) {
        let mut kids = self.kids.children(parent).to_vec();
        kids.sort_by(|a, b| {
            (a.started.unwrap_or(0.0), a.seq)
                .partial_cmp(&(b.started.unwrap_or(0.0), b.seq))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        kids.into_iter().partition(|a| self.is_busy(a))
    }

    /// Everything below `a`, depth first.
    pub fn descendants(&self, a: &Agent) -> Vec<&'a Agent> {
        self.kids.descendants(&a.id)
    }

    /// `a` and everything below it.
    pub fn subtree(&self, a: &'a Agent) -> Vec<&'a Agent> {
        self.kids.subtree(a)
    }

    /// The glyph and status `a`'s subtree shows (V11 `tState`): its most urgent live agent,
    /// itself included, else idle.
    pub fn state(&self, a: &'a Agent, frame: f64) -> (String, Status) {
        match self
            .subtree(a)
            .into_iter()
            .filter(|a| a.status.is_live())
            .min_by_key(|a| a.status.urgency())
        {
            Some(u) => (glyph(u, frame), u.status.clone()),
            None => ("○".into(), Status::Queued),
        }
    }
}

/// Marks `a` busy when it or a descendant is live; whether it is.
fn mark_busy<'a>(
    a: &'a Agent,
    kids: &TreeKids<'a>,
    depth: usize,
    busy: &mut HashSet<&'a str>,
) -> bool {
    let mut b = a.status.is_live();
    if depth < MAX_DEPTH {
        for k in kids.children(Some(&a.id)) {
            b |= mark_busy(k, kids, depth + 1, busy);
        }
    }
    if b {
        busy.insert(&a.id);
    }
    b
}
