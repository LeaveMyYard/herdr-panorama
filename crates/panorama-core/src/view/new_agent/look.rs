//! The modal's colours (the design's palette `C`, fixed values, not herdr's theme) and its
//! drawing primitives as the design's `Scr` draws: a fill blanks cells, art is drawn without
//! its spaces, a box is a border with an optional inside.

use crate::view::Screen;
use crate::view::colour::Rgb;
use crate::view::seg::Seg;

pub const BG: Rgb = Rgb::hex(0x101215);
pub const PANEL: Rgb = Rgb::hex(0x171a1f);
pub const BORDER: Rgb = Rgb::hex(0x80868f);
pub const TEXT: Rgb = Rgb::hex(0xeceef1);
pub const MUTED: Rgb = Rgb::hex(0x80868f);
pub const DIM: Rgb = Rgb::hex(0x4d535c);
pub const FAINT: Rgb = Rgb::hex(0x2c313a);
pub const ACCENT: Rgb = Rgb::hex(0xe2cf9c);
pub const WARN: Rgb = Rgb::hex(0xffb547);
pub const FIELD: Rgb = Rgb::hex(0x1d2127);
/// A neighbour's art in the carousel.
pub const GHOST_ART: Rgb = Rgb::hex(0x2e333b);
/// "Coming soon" under a neighbour's art.
pub const GHOST_TAG: Rgb = Rgb::hex(0x3a3f47);

/// The background of a `w` × `h` box at `(x, y)`, its characters blanked (`Scr.fill`).
pub fn fill(s: &mut Screen, x: i32, y: i32, w: i32, h: i32, col: Rgb) {
    for j in y..y + h {
        for i in x..x + w {
            if let Some(c) = s.cell_mut(i, j) {
                c.bg = Some(col);
                c.ch = ' ';
            }
        }
    }
}

/// Lines of art from `(x, y)`; a space leaves the cell under it (`Scr.art`).
pub fn art(s: &mut Screen, x: i32, y: i32, lines: &[&str], fg: Rgb, bold: bool) {
    for (j, line) in (0..).zip(lines) {
        for (i, ch) in (0..).zip(line.chars()) {
            if ch != ' ' {
                s.put(x + i, y + j, ch.encode_utf8(&mut [0; 4]), fg, None, bold);
            }
        }
    }
}

/// A box's border style.
#[derive(Clone, Copy)]
pub enum Border {
    Light,
    Round,
}

/// A `w` × `h` box at `(x, y)`: `bg` under all of it, `inner` under its inside (`Scr.box`).
pub fn boxed(
    s: &mut Screen,
    (x, y, w, h): (i32, i32, i32, i32),
    col: Rgb,
    bg: Option<Rgb>,
    style: Border,
    inner: Option<Rgb>,
) {
    let [tl, tr, bl, br, hz, vt] = match style {
        Border::Light => ['┌', '┐', '└', '┘', '─', '│'],
        Border::Round => ['╭', '╮', '╰', '╯', '─', '│'],
    };
    if let Some(bg) = bg {
        fill(s, x, y, w, h, bg);
    }
    if let Some(inner) = inner {
        fill(s, x + 1, y + 1, w - 2, h - 2, inner);
    }
    let rule =
        |l: char, r: char| format!("{l}{}{r}", hz.to_string().repeat((w - 2).max(0) as usize));
    s.put(x, y, &rule(tl, tr), col, bg, false);
    for j in 1..h - 1 {
        s.put(x, y + j, &vt.to_string(), col, bg, false);
        s.put(x + w - 1, y + j, &vt.to_string(), col, bg, false);
    }
    s.put(x, y + h - 1, &rule(bl, br), col, bg, false);
}

/// A segment from `(x, y)`, cut before column `end`; the x after what was drawn.
pub fn put_upto(s: &mut Screen, x: i32, y: i32, end: i32, seg: Seg) -> i32 {
    let room = (end - x).max(0) as usize;
    let text = seg.text.chars().take(room).collect();
    s.put_seg(x, y, &Seg { text, ..seg })
}

/// Characters in `text` (a cell each).
pub fn len(text: &str) -> i32 {
    text.chars().count() as i32
}
