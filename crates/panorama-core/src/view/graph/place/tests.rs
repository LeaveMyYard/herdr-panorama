//! The tree's placement in its depth columns.

use super::{Placed, place_tree};
use crate::view::nav::TreeEntry;

fn entry(ident: &str, depth: usize, key: &str) -> TreeEntry {
    TreeEntry {
        ident: ident.into(),
        depth,
        key: key.into(),
        fin: vec![],
    }
}

#[test]
fn children_start_level_with_their_parent_and_groups_never_share_rows() {
    // t: a (spawned a1, a2), b (spawned b1); every card 3 rows
    let entries = [
        entry("a", 1, "t"),
        entry("a1", 2, "a"),
        entry("a2", 2, "a"),
        entry("b", 1, "t"),
        entry("b1", 2, "b"),
    ];
    let tree = place_tree(&entries, |_| 3);
    let ys: Vec<i32> = tree.cards.iter().map(|p| p.y).collect();
    // a1 level with a, a2 below it; b below a in its column, and below the row a2's link
    // joins, so the two groups' trunks never share rows; b1 level with b (Python's numbers)
    assert_eq!(ys, [0, 0, 4, 6, 8]);
    assert_eq!(tree.end, 12);
    let groups: Vec<(Option<usize>, usize, Vec<i32>)> = tree
        .groups
        .iter()
        .map(|g| (g.parent.map(|p| p.entry), g.depth, g.anchors.clone()))
        .collect();
    assert_eq!(
        groups,
        [
            (None, 1, vec![1, 7]),
            (Some(0), 2, vec![1, 5]),
            (Some(3), 2, vec![9])
        ]
    );
}

#[test]
fn a_tall_card_pushes_the_cards_below_it() {
    let entries = [entry("a", 1, "t"), entry("b", 1, "t")];
    let tree = place_tree(&entries, |e| if e.ident == "a" { 8 } else { 3 });
    assert_eq!(
        tree.cards,
        [
            Placed {
                entry: 0,
                y: 0,
                h: 8
            },
            Placed {
                entry: 1,
                y: 9,
                h: 3
            }
        ]
    );
    assert_eq!(tree.end, 13);
}

#[test]
fn no_entries_no_height() {
    let tree = place_tree(&[], |_| 3);
    assert!(tree.cards.is_empty() && tree.groups.is_empty());
    assert_eq!(tree.end, 0);
}
