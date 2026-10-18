//! Card borders and the lines that join cards (Python `render.border`, `rbox`, `jn`).

use crate::view::colour::Rgb;
use crate::view::palette::{DIM, FAINT, HI, LINE};
use crate::view::screen::Screen;
use crate::view::state::View;
use crate::view::target::Target;

/// The border of an unselected card under the mouse.
pub const HOVER: Rgb = FAINT;

/// A card's border colour: focused, selected, under the mouse, or plain.
pub fn border(view: &View, card: &Target, focus: bool, is_sel: bool) -> Rgb {
    if focus {
        HI
    } else if is_sel {
        DIM
    } else if view.hover.as_ref() == Some(card) {
        HOVER
    } else {
        LINE
    }
}

/// A rounded box, `w` × `h`, at `(x, y)`.
pub fn rbox(s: &mut Screen, x: i32, y: i32, w: i32, h: i32, col: Rgb) {
    let inner = "─".repeat((w - 2).max(0) as usize);
    s.put(x, y, &format!("╭{inner}╮"), col, None, false);
    for j in 1..h - 1 {
        s.put(x, y + j, "│", col, None, false);
        s.put(x + w - 1, y + j, "│", col, None, false);
    }
    s.put(x, y + h - 1, &format!("╰{inner}╯"), col, None, false);
}

/// The line glyph for directions up 1, down 2, left 4, right 8 (rounded corners).
fn bx(bits: u8) -> Option<char> {
    Some(match bits {
        1..=3 => '│',
        12 | 4 | 8 => '─',
        10 => '╭',
        9 => '╰',
        6 => '╮',
        5 => '╯',
        11 => '├',
        7 => '┤',
        14 => '┬',
        13 => '┴',
        15 => '┼',
        _ => return None,
    })
}

/// The directions a line glyph already has.
fn xb(ch: char) -> Option<u8> {
    Some(match ch {
        '│' => 3,
        '─' => 12,
        '╭' | '┌' => 10,
        '╰' | '└' => 9,
        '╮' | '┐' => 6,
        '╯' | '┘' => 5,
        '├' => 11,
        '┤' => 7,
        '┬' => 14,
        '┴' => 13,
        '┼' => 15,
        _ => return None,
    })
}

/// Adds the directions `bits` to the line at `(x, y)` (design turn 3): a line glyph merges
/// (and keeps its colour), anything else becomes the line in `col`. An animated card edge
/// `┃` is joined as `┨` from the left or `┠` to the right.
pub fn jn(s: &mut Screen, x: i32, y: i32, bits: u8, col: Rgb) {
    let Some(c) = s.cell_mut(x, y) else { return };
    if c.ch == '┃' {
        match bits {
            4 => c.ch = '┨',
            8 => c.ch = '┠',
            _ => {}
        }
        return;
    }
    if let Some(had) = xb(c.ch) {
        if let Some(ch) = bx(had | bits) {
            c.ch = ch;
        }
    } else if let Some(ch) = bx(bits) {
        c.ch = ch;
        c.fg = col;
        c.fg_moves = false;
        c.bold = false;
    }
}
