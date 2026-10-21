//! tests/test_mouse.py's wheel: an overflowing column scrolls and keeps the selection; a click
//! right after a scroll does not hit what slid under the pointer.

use super::*;

#[test]
fn the_wheel_scrolls_an_overflowing_column_and_keeps_the_selection() {
    let mut r = Run::demo("w1:p1", (160, 16)); // short: the orchestrators overflow
    let now = r.settled();
    let before = format!("{:?}", r.sel());
    let (x, y) = r.spot(&orch("w1:p1")).unwrap();
    let wheel = |r: &mut Run, down| {
        if let (_, Some(cause)) =
            r.d.pointer(Pointer::Wheel { down, shift: false }, x, y, now)
        {
            r.paint(cause, now);
        }
    };
    wheel(&mut r, true);
    assert_eq!(r.d.app.view.offsets.get(&0), Some(&3));
    assert_eq!(format!("{:?}", r.sel()), before);
    r.refresh(wire("demo").0, now); // a refresh does not scroll it back
    assert_eq!(r.d.app.view.offsets.get(&0), Some(&3));
    for _ in 0..20 {
        wheel(&mut r, true);
    }
    let off = r.d.app.view.offsets[&0];
    assert!(3 < off && off <= 20, "{off}"); // clamped to the content
    wheel(&mut r, false);
    r.key(Key::Move(1), now); // a selection change follows the cursor again
    assert!(r.d.app.view.wheel.is_empty());
}

#[test]
fn a_click_right_after_a_wheel_scroll_does_not_hit_what_slid_under_the_pointer() {
    let mut r = Run::demo("w1:p1", (160, 16));
    let now = r.settled();
    let (x, y) = r.spot(&orch("w1:p1")).unwrap();
    let before = r.d.shown().unwrap().clone();
    let (_, cause) = r.d.pointer(
        Pointer::Wheel {
            down: true,
            shift: false,
        },
        x,
        y,
        now,
    );
    r.paint(cause.unwrap(), now);
    let after = r.d.shown().unwrap().clone();
    let moved = after
        .targets
        .iter()
        .find(|h| {
            matches!(h.target, Target::Orch(_)) && before.target_at(h.x0, h.y) != Some(&h.target)
        })
        .unwrap()
        .clone();
    let Target::Orch(pane) = &moved.target else {
        unreachable!()
    };
    r.click_at((moved.x0, moved.y), now + Duration::from_millis(100));
    assert_eq!(r.d.app.view.orch.as_deref(), Some("w1:p1")); // ignored
    r.click_at((moved.x0, moved.y), now + Duration::from_millis(400)); // the window passed
    assert_eq!(r.d.app.view.orch.as_deref(), Some(pane.as_str()));
}

#[test]
fn the_wheel_outside_a_column_or_on_a_short_one_does_nothing() {
    let mut r = Run::demo("w1:p1", (160, 45));
    let now = r.settled();
    assert_eq!(
        r.d.pointer(
            Pointer::Wheel {
                down: true,
                shift: false
            },
            5,
            0,
            now
        )
        .1,
        None
    ); // the header
    let (x, y) = r.spot(&orch("w1:p1")).unwrap();
    if let (_, Some(cause)) = r.d.pointer(
        Pointer::Wheel {
            down: true,
            shift: false,
        },
        x,
        y,
        now,
    ) {
        r.paint(cause, now);
    }
    assert_eq!(r.d.app.view.offsets.get(&0).copied().unwrap_or(0), 0); // it fits: clamped to 0
}
