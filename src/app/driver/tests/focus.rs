//! tests/test_focus_theme.py's look: the docked panel shows whether it has the keyboard, the
//! graph does not.

use super::*;
use panorama_core::view::theme::FIXED;

/// An empty row of the docked panel (Python `c[30][20]`).
fn bg(r: &mut Run) -> Option<panorama_core::view::Rgb> {
    r.paint(Painted::Other, r.t0);
    r.d.shown().unwrap().cell(20, 30).bg
}

fn docked(size: (i32, i32)) -> Run {
    let (mut sw, now) = wire("design9");
    sw.docked = Some("w1:p3".into());
    Run::new((sw, now), "w1:p3", true, size)
}

#[test]
fn the_view_follows_the_terminals_focus_in_and_out() {
    let (focused, unfocused) = FIXED; // no config: the fixed default
    let mut r = docked((46, 45));
    assert!(r.d.app.dock && bg(&mut r) == Some(focused));
    assert!(r.d.focus(false)); // the terminal's focus went elsewhere
    assert_eq!(bg(&mut r), Some(unfocused));
    assert!(!r.d.focus(false)); // no change: no repaint needed
    assert!(r.d.focus(true));
    assert_eq!(bg(&mut r), Some(focused));
    // the wide graph has one look
    let mut wide = docked((160, 45));
    assert!(!wide.d.app.dock);
    let before = bg(&mut wide);
    wide.d.focus(false);
    assert_eq!(bg(&mut wide), before);
}
