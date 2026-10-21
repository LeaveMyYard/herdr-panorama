//! A text field as the design's `input()` draws it: the label, a light box (accent when
//! focused) filled with the field colour, the value, the cursor cell on the focused field, and
//! a sub line under the box. A value longer than the box scrolls so the cursor stays in it.

use super::look::{ACCENT, BG, Border, DIM, FIELD, MUTED, TEXT, boxed, put_upto};
use crate::view::Screen;
use crate::view::colour::Rgb;
use crate::view::seg::Seg;

/// One field to draw.
pub struct Input<'a> {
    pub label: &'a str,
    pub value: &'a str,
    /// The cursor (a character index into `value`) when the field has the focus.
    pub cursor: Option<usize>,
    /// The line under the box: `(text, colour)` segments.
    pub sub: &'a [(&'a str, Rgb)],
}

/// Rows a field takes, its sub line included, before the one-row gap to the next.
pub const ROWS: i32 = 5;

/// The field `w` wide at `(x, y)`.
pub fn draw(s: &mut Screen, x: i32, y: i32, w: i32, f: &Input<'_>) {
    let focus = f.cursor.is_some();
    s.put(x, y, f.label, if focus { TEXT } else { MUTED }, None, focus);
    boxed(
        s,
        (x, y + 1, w, 3),
        if focus { ACCENT } else { DIM },
        None,
        Border::Light,
        Some(FIELD),
    );
    // the value from x + 2 up to the box's last inside column, the cursor's cell included
    let (start, end) = (x + 2, x + w - 1);
    let chars: Vec<char> = f.value.chars().collect();
    let room = (end - start).max(1) as usize;
    let first = f
        .cursor
        .map_or(0, |c| (c.min(chars.len()) + 1).saturating_sub(room));
    let shown: String = chars[first.min(chars.len())..].iter().collect();
    put_upto(s, start, y + 2, end, Seg::new(shown, TEXT).on(FIELD));
    if let Some(c) = f.cursor {
        let c = c.min(chars.len());
        let ch = chars.get(c).copied().unwrap_or(' ');
        s.put(
            start + (c - first) as i32,
            y + 2,
            ch.encode_utf8(&mut [0; 4]),
            BG,
            Some(TEXT),
            false,
        );
    }
    let mut sx = x + 2;
    for (text, col) in f.sub {
        sx = put_upto(s, sx, y + 4, end, Seg::new(*text, *col));
    }
}
