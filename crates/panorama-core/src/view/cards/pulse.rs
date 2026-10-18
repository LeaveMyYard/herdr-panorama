//! The unseen pulse on a card's whole border (Python `render.pulse_box`, design turn 6).

use super::Area;
use crate::view::anim::pulse;
use crate::view::colour::Rgb;
use crate::view::palette::{DIM, HI};
use crate::view::screen::Screen;
use crate::view::unseen::Mark;

/// The selected card's largest share of the event colour (an unselected card's: all of it).
pub const SELECTED_PULSE: f64 = 0.6;

/// The border of `card` (corners, sides, the animated edge and its sweep) in the pulse
/// colour of `m`, run over `base`, the card's own border colour. A selected card (`base`
/// [`HI`] or [`DIM`]) takes at most [`SELECTED_PULSE`] of the event colour, so it stays
/// readable as selected while it pulses.
pub fn pulse_box(cv: &mut Screen, card: Area, m: Option<Mark>, frame: f64, base: Rgb) {
    let Some(m) = m else { return };
    let cap = if base == HI || base == DIM {
        SELECTED_PULSE
    } else {
        1.0
    };
    let col = pulse(frame, m, base, cap);
    let Area { x, y, w, h } = card;
    let mut paint = |xx: i32, yy: i32| {
        if let Some(c) = cv.cell_mut(xx, yy) {
            c.fg = col;
            c.fg_moves = true;
        }
    };
    for i in 0..w {
        paint(x + i, y);
        paint(x + i, y + h - 1);
    }
    for j in 1..h - 1 {
        paint(x, y + j);
        paint(x + w - 1, y + j);
    }
}
