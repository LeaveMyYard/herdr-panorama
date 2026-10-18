//! The agent tree laid out in its depth columns (Python `render.place_tree`).
//!
//! Each column stacks its cards. A group of children starts level with the agent that
//! spawned it (or below the cards already in its column), and an agent with children starts
//! below the previous group's last child, so two groups' connector lines never share rows.

use crate::view::nav::TreeEntry;
use std::collections::{BTreeMap, HashMap, HashSet};

/// A card as placed: its entry's index, its first row and its height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    pub entry: usize,
    pub y: i32,
    pub h: i32,
}

/// A group of siblings: the card that spawned them (`None`: the thread's own), their depth,
/// and the row each card's link joins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub parent: Option<Placed>,
    pub depth: usize,
    pub anchors: Vec<i32>,
}

/// The tree as placed: the cards in entry order, the groups in order of their first card,
/// and the height.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tree {
    pub cards: Vec<Placed>,
    pub groups: Vec<Group>,
    pub end: i32,
}

/// The index of each entry's parent: the card of the agent whose group it is in.
fn parents(entries: &[TreeEntry]) -> Vec<Option<usize>> {
    let mut by_ident: HashMap<&str, usize> = HashMap::new();
    entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let p = (e.depth > 1)
                .then(|| by_ident.get(e.key.as_str()).copied())
                .flatten();
            by_ident.insert(e.ident.as_str(), i);
            p
        })
        .collect()
}

/// Places `entries` (depth first), each `height(entry)` rows tall.
pub fn place_tree(entries: &[TreeEntry], height: impl Fn(&TreeEntry) -> i32) -> Tree {
    let parent = parents(entries);
    let has_kids: HashSet<usize> = parent.iter().flatten().copied().collect();
    let mut cur: BTreeMap<usize, i32> = BTreeMap::new();
    let mut trunk_end: HashMap<usize, i32> = HashMap::new();
    let mut cards: Vec<Placed> = vec![];
    let mut groups: Vec<(Option<usize>, Group)> = vec![];
    for (i, e) in entries.iter().enumerate() {
        let d = e.depth;
        let y0 = parent[i].map_or(0, |p| cards[p].y);
        let mut y = cur.get(&d).copied().unwrap_or(0).max(y0);
        if has_kids.contains(&i) {
            y = y.max(trunk_end.get(&(d + 1)).copied().unwrap_or(-2) + 1);
        }
        let p = Placed {
            entry: i,
            y,
            h: height(e),
        };
        cards.push(p);
        cur.insert(d, y + p.h + 1);
        let key = (parent[i], d);
        match groups.iter_mut().find(|(k, g)| *k == key.0 && g.depth == d) {
            Some((_, g)) => g.anchors.push(y + 1),
            None => groups.push((
                key.0,
                Group {
                    parent: None,
                    depth: d,
                    anchors: vec![y + 1],
                },
            )),
        }
        let te = trunk_end.entry(d).or_insert(-1);
        *te = (*te).max(y + 1);
    }
    let groups = groups
        .into_iter()
        .map(|(p, g)| Group {
            parent: p.map(|p| cards[p]),
            ..g
        })
        .collect();
    let end = cur.values().copied().max().unwrap_or(0);
    Tree { cards, groups, end }
}

#[cfg(test)]
mod tests;
