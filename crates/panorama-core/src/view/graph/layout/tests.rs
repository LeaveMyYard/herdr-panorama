//! The column widths and the window around the focus.

use super::{Columns, DW, col_label, tier, window};

#[test]
fn tiers() {
    // test_wide.py::test_tiers
    for (w, want) in [
        (120, (28, 36, 44)),
        (160, (28, 50, 44)),
        (219, (28, 50, 44)),
        (220, (32, 56, 56)),
        (299, (32, 56, 56)),
        (300, (36, 64, 60)),
        (500, (36, 64, 60)),
    ] {
        assert_eq!(tier(w), want, "{w}");
    }
}

/// The 160-column layout of a five-deep tree: orchestrators, threads, depth 1..5.
fn deep() -> (Vec<i32>, Vec<i32>) {
    (vec![28, 50, DW, DW, DW, DW, DW], vec![0, 6, 7, 7, 7, 7, 7])
}

#[test]
fn the_window_keeps_the_focus_its_parents_and_fills_right_first() {
    let (widths, gaps) = deep();
    // on the threads: orchestrators | threads | depth 1 (and nothing more fits)
    assert_eq!(window(&widths, &gaps, 1, 154, None), (0, 2));
    // on depth 2 (column 3): its parents' column first, then right
    assert_eq!(window(&widths, &gaps, 3, 154, None), (2, 4));
    // on the last column: the ones before it fill the left
    assert_eq!(window(&widths, &gaps, 6, 154, None), (4, 6));
    // wider: everything
    assert_eq!(window(&widths, &gaps, 3, 400, None), (0, 6));
}

#[test]
fn the_last_column_may_shrink_to_dmin_to_fit() {
    let (widths, gaps) = deep();
    // 28 + 6 + 50 + 7 + DMIN = 127: the third column fits only shrunk
    assert_eq!(window(&widths, &gaps, 0, 127, None), (0, 2));
    assert_eq!(window(&widths, &gaps, 0, 126, None), (0, 1));
}

#[test]
fn a_sideways_scroll_fixes_the_first_column_and_is_clamped() {
    let (widths, gaps) = deep();
    assert_eq!(window(&widths, &gaps, 6, 154, Some(0)), (0, 2));
    assert_eq!(window(&widths, &gaps, 0, 154, Some(5)), (5, 6));
    assert_eq!(window(&widths, &gaps, 0, 154, Some(99)), (6, 6));
}

#[test]
fn placed_columns_keep_their_width_and_the_last_shrinks_never_stretches() {
    let (widths, _) = deep();
    let c = Columns::place(widths.clone(), 160, 1, None);
    assert_eq!((c.lo, c.hi), (0, 2));
    assert_eq!([c.x(0), c.x(1), c.x(2)], [2, 36, 93]);
    assert_eq!(c.w(2), DW); // room for 44, not stretched to the edge
    assert!(c.sideways());
    // scrolled: two more columns on the left for the lines in from there
    let c = Columns::place(widths.clone(), 120, 5, None);
    assert_eq!((c.lo, c.hi, c.x(4), c.x(5), c.w(5)), (4, 5, 4, 55, DW));
    // at 120 columns the agents' column shrinks to what is left (Python's own numbers)
    let c = Columns::place(vec![28, 36, DW, DW, DW], 120, 1, None);
    assert_eq!((c.lo, c.hi, c.x(1), c.x(2), c.w(2)), (0, 2, 36, 79, 39));
    let c = Columns::place(vec![28], 160, 3, None);
    assert_eq!((c.lo, c.hi, c.x(0)), (0, 0, 2));
    assert!(!c.sideways());
}

#[test]
fn column_labels() {
    assert_eq!(col_label(0), "orchestrators");
    assert_eq!(col_label(1), "threads");
    assert_eq!(col_label(4), "depth 3");
}
