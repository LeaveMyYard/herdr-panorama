//! tests/test_dock_panel.py's home view docked next to an agent: the docked panel's one
//! column, and a card a refresh moved under the pointer.

use super::*;
use panorama_core::view::NEW_ROW;

fn docked(pane: &str) -> (Swarm, f64) {
    let (mut sw, now) = wire("design9");
    sw.docked = Some(pane.into());
    (sw, now)
}

#[test]
fn the_docked_view_uses_the_panel_and_keeps_one_column() {
    let mut r = Run::new(docked("w1:p3"), "w1:p3", true, (46, 45));
    assert!(r.d.app.dock && r.text().contains("▌ 3 ")); // the docked entry
    let now = r.settled();
    r.key(Key::Right, now);
    assert_eq!(r.d.app.view.focus, 0); // one column: → does not leave it
    r.key(Key::Move(1), now);
    assert_eq!(r.d.app.view.orch.as_deref(), Some("w1:p4"));
    r.key(Key::Move(1), now); // design 9's dataset has a fifth tab (Python's test data four)
    assert_eq!(r.d.app.view.orch.as_deref(), Some("w1:p5"));
    r.key(Key::Move(1), now);
    assert_eq!(r.d.app.view.orch.as_deref(), Some(NEW_ROW));
}

#[test]
fn a_card_that_moved_ignores_clicks() {
    let mut r = Run::new(docked("w1:p3"), "w1:p3", true, (46, 45));
    let now = r.settled();
    let at = r.spot(&orch("w1:p2")).unwrap();
    // docked elsewhere: Payments UI's entry grows, Data Pipeline moves down
    r.refresh(docked("w1:p1").0, now);
    assert_eq!(
        r.d.shown().unwrap().target_at(at.0, at.1),
        Some(&orch("w1:p1"))
    );
    r.click_at(at, now + Duration::from_millis(100));
    assert_eq!(r.d.app.view.orch.as_deref(), Some("w1:p3")); // ignored: it just arrived
}
