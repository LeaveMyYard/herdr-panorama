use super::*;

fn screen(targets: &[(i32, &str)]) -> Screen {
    let mut s = Screen::new(10, 3);
    for (y, pane) in targets {
        s.target(0, 10, *y, Target::Orch((*pane).into()));
    }
    s
}

#[test]
fn a_cell_whose_target_changed_in_a_refresh_waits_a_second() {
    let t0 = Instant::now();
    let mut a = ClickArming::default();
    a.refreshed(&screen(&[(0, "p1")]), &screen(&[(0, "p1"), (1, "p2")]), t0);
    assert!(a.ready((3, 0), t0)); // unchanged
    assert!(!a.ready((3, 1), t0 + Duration::from_millis(999)));
    assert!(a.ready((3, 1), t0 + CLICK_ARM));
}

#[test]
fn the_wheel_holds_moved_and_vanished_cells_briefly() {
    let t0 = Instant::now();
    let mut a = ClickArming::default();
    a.scrolled(&screen(&[(0, "p1"), (2, "p3")]), &screen(&[(0, "p2")]), t0);
    assert!(!a.ready((0, 0), t0));
    assert!(!a.ready((0, 2), t0 + Duration::from_millis(299)));
    assert!(a.ready((0, 2), t0 + SCROLL_ARM));
}
