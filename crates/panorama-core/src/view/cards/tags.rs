//! A card's tags on its bottom border, right-aligned (Python `render.tags`): ` age ` and the
//! like, a border cell apart.

use crate::view::palette::DIM;
use crate::view::screen::Screen;
use crate::view::seg::Seg;

/// The non-empty `items` on row `y` of the `w`-wide card at `x`, the last one nearest the
/// right corner.
pub fn tags(s: &mut Screen, x: i32, y: i32, w: i32, items: &[Option<String>]) {
    let mut xx = x + w - 2;
    for t in items.iter().rev().flatten().filter(|t| !t.is_empty()) {
        xx -= t.chars().count() as i32 + 2;
        s.segs(xx, y, &[Seg::new(format!(" {t} "), DIM)]);
        xx -= 1;
    }
}
