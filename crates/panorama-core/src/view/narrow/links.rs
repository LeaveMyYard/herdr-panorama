//! The lines from the selected orchestrator down to its cards (Python `_narrow`'s anchors):
//! a trunk at column 1 from below the orchestrator to the last card, a branch to each.

use crate::view::cards::boxes::jn;
use crate::view::palette::LINE;
use crate::view::screen::Screen;

/// Joins the orchestrator at row `top_y` to the cards whose link rows are `anchors`.
pub fn join(cv: &mut Screen, top_y: i32, anchors: &[i32]) {
    let Some(&last) = anchors.last() else { return };
    for yy in top_y + 1..=last {
        let bits = 1 | if yy < last { 2 } else { 0 } | if anchors.contains(&yy) { 8 } else { 0 };
        jn(cv, 1, yy, bits, LINE);
    }
    for &ay in anchors {
        jn(cv, 2, ay, 12, LINE);
        if matches!(cv.cell(3, ay).ch, '│' | '┃') {
            jn(cv, 3, ay, 4, LINE);
        }
    }
}
