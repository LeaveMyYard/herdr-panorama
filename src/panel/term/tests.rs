use super::*;
use panorama_core::view::palette::{FG, HI};
use ratatui::layout::Rect;

#[test]
fn a_screen_lands_cell_for_cell_with_panoramas_background() {
    let mut s = Screen::new(3, 2);
    s.put(0, 0, "a✓", HI, Some(Rgb::hex(0x123456)), true);
    let mut buf = Buffer::empty(Rect::new(0, 0, 3, 2));
    copy(&s, &mut buf);
    let a = &buf[(0, 0)];
    assert_eq!(
        (a.symbol(), a.fg, a.bg),
        ("a", colour(HI), Color::Rgb(0x12, 0x34, 0x56))
    );
    assert!(a.modifier.contains(Modifier::BOLD));
    assert_eq!(buf[(1, 0)].symbol(), "✓");
    let blank = &buf[(2, 1)];
    assert_eq!(
        (blank.symbol(), blank.fg, blank.bg),
        (" ", colour(FG), colour(BG))
    );
    assert!(!blank.modifier.contains(Modifier::BOLD));
}
