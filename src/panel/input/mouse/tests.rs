use crate::panel::input::Effect;
use crate::panel::input::fixtures::panel;
use crate::panel::paint::Frame;
use panorama_core::view::Target;
use std::time::Duration;

fn frame() -> Frame {
    Frame {
        now: 0.0,
        frame: 0.0,
        utc_offset: 0,
        theme: None,
        settings: Default::default(),
        open_key: ("pfx+spc".into(), "pfx+spc".into()),
    }
}

/// Where `target` is drawn on `s`.
fn at(s: &panorama_core::view::Screen, target: &Target) -> (i32, i32) {
    let hit = s
        .targets
        .iter()
        .find(|h| &h.target == target)
        .expect("drawn");
    (hit.x0, hit.y)
}

#[test]
fn a_click_on_an_entry_in_the_docked_list_switches_to_it() {
    let (mut p, t) = panel("w1:t2", "w1:t2");
    let s = p.paint(46, 45, &frame());
    let (x, y) = at(&s, &Target::Orch("w1:p4".into()));
    let out = p.click(x, y, &s, t);
    assert!(matches!(&out[..], [Effect::Goto { pane, keep_focus: true, .. }] if pane == "w1:p4"));
}

#[test]
fn a_click_on_a_cell_that_just_changed_does_nothing() {
    let (mut p, t) = panel("w1:t2", "w1:t2");
    let old = panorama_core::view::Screen::new(46, 45);
    let s = p.paint(46, 45, &frame());
    p.input.clicks.refreshed(&old, &s, t);
    let (x, y) = at(&s, &Target::Orch("w1:p4".into()));
    assert!(p.click(x, y, &s, t + Duration::from_millis(500)).is_empty());
    assert!(!p.click(x, y, &s, t + Duration::from_secs(2)).is_empty());
}

#[test]
fn narrow_a_click_selects_and_a_second_goes_to_the_pane() {
    let (mut p, t) = panel("w1:t9", "w1:t1");
    let s = p.paint(46, 45, &frame());
    let (x, y) = at(&s, &Target::Orch("w1:p2".into()));
    assert!(p.click(x, y, &s, t).is_empty());
    assert_eq!(p.view.orch.as_deref(), Some("w1:p2"));
    let s = p.paint(46, 45, &frame());
    let (x, y) = at(&s, &Target::Orch("w1:p2".into()));
    assert!(
        matches!(&p.click(x, y, &s, t)[..], [Effect::Goto { pane, keep_focus: false, .. }] if pane == "w1:p2")
    );
}

#[test]
fn the_hover_follows_the_mouse_and_the_wheel_scrolls_the_list() {
    let (mut p, _) = panel("w1:t2", "w1:t2");
    let s = p.paint(46, 45, &frame());
    let (x, y) = at(&s, &Target::Orch("w1:p4".into()));
    assert!(p.hover(Some((x, y)), &s));
    assert_eq!(p.view.hover, Some(Target::Orch("w1:p4".into())));
    assert!(!p.hover(Some((x, y)), &s)); // no change
    assert!(p.hover(None, &s));
    assert!(p.wheel(5, 5, 3, &s));
    assert_eq!(p.view.offsets.get(&0), Some(&3));
    assert!(p.view.wheel.contains(&0));
    assert!(!p.wheel(5, 44, 3, &s)); // the footer is no column
}
