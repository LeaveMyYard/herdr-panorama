//! The tree rows under a thread in the docked entry (Python `render._spawn_rows`): each agent
//! that spawned others and is live or has a live descendant, depth first, with its prefix.

use crate::model::{Agent, TreeKids};
use std::collections::HashMap;

/// Deeper than this, a tree is cut (a guard; parent loops are already cut by `TreeKids`).
const MAX_DEPTH: usize = 64;

/// One row: the prefix drawn before its branch, whether it is its parent's last such child,
/// and the agent.
pub struct SpawnRow<'a> {
    pub prefix: String,
    pub last: bool,
    pub agent: &'a Agent,
}

/// The rows of a thread's tree, depth first, siblings by start time then `seq` (a status change never
/// reorders them).
pub fn spawn_rows<'a>(kids: &TreeKids<'a>) -> Vec<SpawnRow<'a>> {
    let mut busy: HashMap<&str, bool> = HashMap::new();
    let mut out = vec![];
    rec(kids, None, String::new(), 0, &mut busy, &mut out);
    out
}

fn is_busy<'a>(
    a: &'a Agent,
    kids: &TreeKids<'a>,
    depth: usize,
    memo: &mut HashMap<&'a str, bool>,
) -> bool {
    if let Some(&b) = memo.get(a.id.as_str()) {
        return b;
    }
    memo.insert(&a.id, false);
    let b = a.status.is_live()
        || (depth < MAX_DEPTH
            && kids
                .children(Some(&a.id))
                .iter()
                .any(|k| is_busy(k, kids, depth + 1, memo)));
    memo.insert(&a.id, b);
    b
}

fn rec<'a>(
    kids: &TreeKids<'a>,
    parent: Option<&str>,
    prefix: String,
    depth: usize,
    memo: &mut HashMap<&'a str, bool>,
    out: &mut Vec<SpawnRow<'a>>,
) {
    if depth > MAX_DEPTH {
        return;
    }
    let mut spawners: Vec<&'a Agent> = kids
        .children(parent)
        .iter()
        .copied()
        .filter(|a| kids.has_children(&a.id) && is_busy(a, kids, 0, memo))
        .collect();
    spawners.sort_by(|a, b| {
        (a.started.unwrap_or(0.0), a.seq)
            .partial_cmp(&(b.started.unwrap_or(0.0), b.seq))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let n = spawners.len();
    for (i, a) in spawners.into_iter().enumerate() {
        let last = i + 1 == n;
        out.push(SpawnRow {
            prefix: prefix.clone(),
            last,
            agent: a,
        });
        let deeper = format!("{prefix}{}", if last { "  " } else { "│ " });
        rec(kids, Some(&a.id), deeper, depth + 1, memo, out);
    }
}
