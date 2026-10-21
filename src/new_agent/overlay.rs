//! The modal over the view (Python's `NewAgentForm` CSS: the view behind dimmed by
//! `rgba(16, 18, 21, 0.55)`, the 1C modal centred on top).

use panorama_core::view::Screen;
use panorama_core::view::colour::Rgb;
use panorama_core::view::new_agent::{H, W};

/// The dimming colour and how much of it covers the view.
pub const SHADE: Rgb = Rgb::hex(0x101215);
pub const SHADE_ALPHA: f64 = 0.55;

/// `view` dimmed, with `modal` (the 1C modal rendered at the view's size) on top.
pub fn over(view: &mut Screen, modal: &Screen) {
    let (w, h) = (view.w(), view.h());
    for y in 0..h {
        for x in 0..w {
            // every cell, drawn on or not: an undrawn one takes the shade
            let Some(c) = view.cell_mut(x, y) else {
                continue;
            };
            c.fg = c.fg.mix(SHADE, SHADE_ALPHA);
            c.bg = Some(c.bg.map_or(SHADE, |b| b.mix(SHADE, SHADE_ALPHA)));
            (c.fg_moves, c.bg_moves) = (false, false); // nothing behind the modal animates
        }
    }
    place(view, modal);
    view.targets.clear(); // the view behind takes no clicks
}

/// The modal's cells (centred as `render` centres it) onto `s`.
pub fn place(s: &mut Screen, modal: &Screen) {
    let (w, h) = (s.w(), s.h());
    let (mx, my) = ((w - W).max(0) / 2, (h - H).max(0) / 2);
    for y in my..(my + H).min(h) {
        for x in mx..(mx + W).min(w) {
            if let Some(c) = s.cell_mut(x, y) {
                *c = modal.cell(x, y);
            }
        }
    }
}
