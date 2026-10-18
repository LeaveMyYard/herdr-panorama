//! The key hints on the last row.

use crate::view::palette::{FAINT, FG};
use crate::view::screen::Screen;
use crate::view::seg::Seg;

const WIDE_KEYS: [(&str, &str); 9] = [
    ("↑↓", "move"),
    ("→", "open"),
    ("←", "back"),
    ("enter", "details"),
    ("g", "go to pane"),
    ("a", "answer"),
    ("i", "inbox"),
    ("f", "filter"),
    ("?", "help"),
];
const NARROW_KEYS: [(&str, &str); 6] = [
    ("↑↓", "move"),
    ("→", "open"),
    ("g", "pane"),
    ("a", "answer"),
    ("f", "filter"),
    ("?", "help"),
];

/// The wide layout's hint when some of its columns are off-screen, after its third.
const SIDEWAYS: (&str, &str) = ("shift+wheel", "scroll");

/// The key hints on the last row, the wide layout's or the narrow one's (no `i` without the
/// inbox; `shift+wheel scroll` when the wide layout scrolls `sideways`; `z panel` when
/// zoomable).
pub fn keys(s: &mut Screen, wide: bool, sideways: bool, zoom_hint: bool, inbox_ok: bool) {
    let mut items: Vec<(&str, &str)> = if wide {
        &WIDE_KEYS[..]
    } else {
        &NARROW_KEYS[..]
    }
    .to_vec();
    if wide && sideways {
        items.insert(3, SIDEWAYS);
    }
    if zoom_hint {
        items.push(("z", "panel"));
    }
    let (y, mut x) = (s.h() - 1, 1);
    for (k, lab) in items.into_iter().filter(|(k, _)| *k != "i" || inbox_ok) {
        x = s.segs(
            x,
            y,
            &[Seg::new(k, FG).bold(), Seg::new(format!(" {lab}"), FAINT)],
        ) + 3;
    }
}
