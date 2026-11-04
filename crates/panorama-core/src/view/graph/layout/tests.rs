//! The column widths and the window around the focus.

use super::{Columns, DW, col_label, tier, window};

#[test]
fn tiers() {
    for (w, want) in [
        (120, (40, 44)),
        (160, (40, 44)),
        (219, (40, 44)),
        (220, (44, 56)),
        (299, (44, 56)),
        (300, (48, 60)),
        (500, (48, 60)),
    ] {
        assert_eq!(tier(w), want, "{w}");
    }
}

/// The 160-column layout of a five-deep tree: the roots, depth 1..5.
fn deep() -> (Vec<i32>, Vec<i32>) {
    (vec![40, DW, DW, DW, DW, DW], vec![0, 7, 7, 7, 7, 7])
}

#[test]
fn the_window_keeps_the_focus_its_parents_and_fills_right_first() {
    let (widths, gaps) = deep();
    // on the roots: roots | depth 1 | depth 2 (and nothing more fits)
    assert_eq!(window(&widths, &gaps, 0, 154, None), (0, 2));
    // on depth 2: its parents' column first, then right
    assert_eq!(window(&widths, &gaps, 2, 154, None), (1, 3));
    // on the last column: the ones before it fill the left
    assert_eq!(window(&widths, &gaps, 5, 154, None), (3, 5));
    // wider: everything
    assert_eq!(window(&widths, &gaps, 2, 400, None), (0, 5));
}

#[test]
fn the_last_column_may_shrink_to_dmin_to_fit() {
    let (widths, gaps) = deep();
    // 40 + 7 + 44 + 7 + DMIN = 134: the third column fits only shrunk
    assert_eq!(window(&widths, &gaps, 0, 134, None), (0, 2));
    assert_eq!(window(&widths, &gaps, 0, 133, None), (0, 1));
}

#[test]
fn a_sideways_scroll_fixes_the_first_column_and_is_clamped() {
    let (widths, gaps) = deep();
    assert_eq!(window(&widths, &gaps, 5, 154, Some(0)), (0, 2));
    assert_eq!(window(&widths, &gaps, 0, 154, Some(4)), (4, 5));
    assert_eq!(window(&widths, &gaps, 0, 154, Some(99)), (5, 5));
}

#[test]
fn placed_columns_keep_their_width_and_the_last_shrinks_never_stretches() {
    let (widths, _) = deep();
    let c = Columns::place(widths.clone(), 160, 0, None);
    assert_eq!((c.lo, c.hi), (0, 2));
    assert_eq!([c.x(0), c.x(1), c.x(2)], [2, 49, 100]);
    assert_eq!(c.w(2), DW); // room for 56, not stretched to the edge
    assert!(c.sideways());
    // scrolled: two more columns on the left for the lines in from there
    let c = Columns::place(widths.clone(), 120, 5, None);
    assert_eq!((c.lo, c.hi, c.x(4), c.x(5), c.w(5)), (4, 5, 4, 55, DW));
    // the last column shown shrinks to what is left
    let c = Columns::place(widths.clone(), 142, 0, None);
    assert_eq!((c.lo, c.hi, c.x(2), c.w(2)), (0, 2, 100, 40));
    let c = Columns::place(vec![40], 160, 3, None);
    assert_eq!((c.lo, c.hi, c.x(0)), (0, 0, 2));
    assert!(!c.sideways());
}

#[test]
fn column_labels_are_depths() {
    assert_eq!(col_label(0), "Depth 0");
    assert_eq!(col_label(4), "Depth 4");
}
