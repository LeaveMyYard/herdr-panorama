//! A solid link between cards in neighbouring columns (Python `render.link`): from a card's
//! right border along its anchor row to a trunk, then into the left border of each card it
//! leads to. The borders it meets turn into tees (`├ … ┤`), an animated edge into `┨`.

use super::boxes::jn;
use crate::view::palette::LINE;
use crate::view::screen::Screen;

/// Where a link runs: from `(fx, fy)` (`fy` `None`: nothing to link from) to the trunk at
/// column `tx`, then into column `to_x` on each row of `ys`. Rows outside `top..bottom` are
/// not linked; with none left, or the source outside, nothing is drawn.
#[derive(Clone, Copy, Debug)]
pub struct Link<'a> {
    pub fx: i32,
    pub fy: Option<i32>,
    pub tx: i32,
    pub to_x: i32,
    pub ys: &'a [i32],
    pub top: i32,
    pub bottom: i32,
}

/// Draws the link on `s`.
pub fn link(s: &mut Screen, l: Link) {
    let inside = |y: i32| (l.top..l.bottom).contains(&y);
    let ys: Vec<i32> = l.ys.iter().copied().filter(|y| inside(*y)).collect();
    let Some(fy) = l.fy.filter(|fy| inside(*fy)) else {
        return;
    };
    if ys.is_empty() {
        return;
    }
    jn(s, l.fx, fy, 8, LINE);
    for x in l.fx + 1..l.tx {
        jn(s, x, fy, 12, LINE);
    }
    let y0 = ys.iter().copied().fold(fy, i32::min);
    let y1 = ys.iter().copied().fold(fy, i32::max);
    for y in y0..=y1 {
        let bits = u8::from(y > y0)
            | if y < y1 { 2 } else { 0 }
            | if y == fy { 4 } else { 0 }
            | if ys.contains(&y) { 8 } else { 0 };
        jn(s, l.tx, y, bits, LINE);
    }
    for &y in &ys {
        for x in l.tx + 1..l.to_x {
            jn(s, x, y, 12, LINE);
        }
        jn(s, l.to_x, y, 4, LINE);
    }
}
