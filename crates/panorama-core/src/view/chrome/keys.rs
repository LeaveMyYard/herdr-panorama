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

/// The key hints on the last row, the wide layout's or the narrow one's (no `i` without the
/// inbox; `z panel` when zoomable).
pub fn keys(s: &mut Screen, wide: bool, zoom_hint: bool, inbox_ok: bool) {
    let base: &[(&str, &str)] = if wide { &WIDE_KEYS } else { &NARROW_KEYS };
    let zoom = [("z", "panel")];
    let items = base.iter().chain(if zoom_hint { &zoom[..] } else { &[] });
    let (y, mut x) = (s.h() - 1, 1);
    for (k, lab) in items.filter(|(k, _)| *k != "i" || inbox_ok) {
        x = s.segs(
            x,
            y,
            &[Seg::new(*k, FG).bold(), Seg::new(format!(" {lab}"), FAINT)],
        ) + 3;
    }
}
