//! A pod: a status as a 3-cell tile, ` g ` in the status's colours (Python `render.pod`,
//! `apod`); the graph's agents and the legend wear them.

use super::anim::glyph;
use super::palette::{HI, tile};
use super::screen::Screen;
use super::seg::Seg;
use crate::model::{Agent, Status};

/// A pod for status `st` with glyph `g` at `(x, y)`; `sel` brackets it.
pub fn pod(s: &mut Screen, x: i32, y: i32, st: &Status, g: &str, sel: bool) {
    let (b, f) = tile(st);
    let text = if g.chars().count() == 1 {
        format!(" {g} ")
    } else {
        format!(" {g}").chars().take(3).collect()
    };
    s.segs(x, y, &[Seg::new(text, f).bold().on(b)]);
    if sel {
        s.put(x - 1, y, "[", HI, None, true);
        s.put(x + 3, y, "]", HI, None, true);
    }
}

/// An agent's pod at the animation time `frame`.
pub fn apod(s: &mut Screen, x: i32, y: i32, a: &Agent, frame: f64, sel: bool) {
    pod(s, x, y, &a.status, &glyph(a, frame), sel);
}
