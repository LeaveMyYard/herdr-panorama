//! Design turn 10 (swarm-focus.js) applied to a rendered panel (Python `theme_panel`): the
//! panel's own background becomes the look's, and on the panel's rows (its background, the
//! docked entry's, the hover's) panorama's text colours become the theme's; the hover row
//! takes the look's hover colour. Tiles, status colours and anything animated stay as drawn.

use crate::view::palette::{BG, DOCK_HOV, SEL};
use crate::view::screen::Screen;
use crate::view::theme::PanelLook;

/// Recolours `s` in place.
pub fn apply(s: &mut Screen, look: &PanelLook) {
    for c in s.cells_mut() {
        let cb = c.bg.unwrap_or(BG);
        if c.bg_moves || ![BG, SEL, DOCK_HOV].contains(&cb) {
            continue;
        }
        if !c.fg_moves
            && let Some((_, to)) = look.fgs.iter().find(|(from, _)| *from == c.fg)
        {
            c.fg = *to;
        }
        if cb == BG {
            c.bg = Some(look.bg);
        } else if cb == DOCK_HOV {
            c.bg = Some(look.hover);
        }
    }
}
