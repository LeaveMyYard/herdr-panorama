//! The actor's decisions (#77 review): a state arms moved click targets, a click on a notice
//! stays there, a stale pane check is dropped.

use super::*;
use crate::panel::act::switch::Switched;
use crate::panel::input::arming::{CLICK_ARM, cells};
use crate::panel::input::fixtures::{frame, panel, state};
use serde_json::json;
use std::time::Duration;

fn driver() -> (Driver, Instant) {
    let (p, t) = panel("w1:t2", "w1:t2");
    let mut d = Driver::new(p);
    d.paint(46, 45, &frame(), Painted::Refresh, t);
    (d, t)
}

/// The design 9 state without its first orchestrator: every row below it moves up.
fn fewer() -> State {
    let mut st = state(json!({"visible_tabs": ["w1:t2"], "focused_pane": "w1:p90"}));
    st.swarm.orchestrators.remove(0);
    st
}

#[test]
fn a_state_holds_the_cells_whose_target_moved_for_a_second() {
    for (wait, acts) in [(Duration::from_millis(600), false), (CLICK_ARM, true)] {
        let (mut d, t) = driver();
        let t = t + CLICK_ARM; // the first paint's own hold is over (the reviewer's fix)
        let old = cells(&d.paint(46, 45, &frame(), Painted::Other, t));
        let cause = d.state(fewer(), t);
        let new = d.paint(46, 45, &frame(), cause, t);
        let (&(x, y), _) = cells(&new)
            .iter()
            .filter(|(c, hit)| {
                old.get(c) != Some(hit) && matches!(hit, panorama_core::view::Target::Orch(_))
            })
            .min_by_key(|(c, _)| (c.1, c.0))
            .expect("a moved row");
        let before = d.panel.view.orch.clone();
        let (out, _) = d.pointer(Pointer::Click, x, y, t + wait);
        let acted = !out.is_empty() || d.panel.view.orch != before;
        assert_eq!(acted, acts, "a click {wait:?} after the state");
    }
}

#[test]
fn a_click_on_a_notice_dismisses_it_and_reaches_nothing_under_it() {
    let (mut d, t) = driver();
    // tall enough to cover cards (a panel's notice is 22 cells wide)
    let text = "move open answer inbox transcript filter help new agent ".repeat(8);
    d.toasts.push(Toast::new(Some("keys"), text, 10), t);
    let screen = d.paint(46, 45, &frame(), Painted::Other, t);
    // a notice cell that has a click target under it
    let under = cells(&d.shown.clone().unwrap());
    let rect = d.toast_rects[0];
    let (x, y) = (rect.y0..rect.y1)
        .flat_map(|y| (rect.x0..rect.x1).map(move |x| (x, y)))
        .find(|c| under.contains_key(c))
        .expect("a target under the notice");
    assert_ne!(
        screen.cell(x, y).bg,
        d.shown.as_ref().unwrap().cell(x, y).bg
    );
    let before = d.panel.view.clone();
    let (out, repaint) = d.pointer(Pointer::Click, x, y, t + CLICK_ARM);
    assert!(
        out.is_empty() && d.panel.view.orch == before.orch && d.panel.view.focus == before.focus
    );
    assert_eq!(repaint, Some(Painted::Other));
    assert_eq!(d.toasts.next_expiry(), None); // gone
    // the next click there reaches the card
    d.paint(46, 45, &frame(), Painted::Other, t);
    let (out, _) = d.pointer(Pointer::Click, x, y, t + CLICK_ARM);
    assert!(
        !out.is_empty() || d.panel.view.orch != before.orch || d.panel.view.focus != before.focus
    );
}

fn checked(pane: &str, nav: u64) -> Done {
    Done::Checked {
        pane: pane.into(),
        label: pane.into(),
        keep_focus: false,
        seen: vec![],
        exists: true,
        nav,
    }
}

#[test]
fn a_pane_check_that_comes_back_after_a_newer_move_is_dropped() {
    // the review's sequence: g on A while herdr is slow, then ↓ to B: A's answer comes late
    let (mut d, t) = driver();
    assert!(matches!(
        &d.key(Key::Goto, t)[..],
        [.., Effect::Goto { .. }]
    ));
    let a = d.launched(); // the actor launches the check
    d.key(Key::Move(1), t);
    assert!(
        d.done(checked("w1:p2", a), t).is_empty(),
        "A's late check switched"
    );
    // nothing newer: the check goes on to the switch
    let (mut d, t) = driver();
    d.key(Key::Goto, t);
    let a = d.launched();
    assert!(
        d.done(checked("w1:p2", a), t)
            .iter()
            .any(|e| matches!(e, Effect::Switch { .. }))
    );
    // a switch that went out meanwhile is newer too
    let (mut d, t) = driver();
    let a = d.launched();
    d.launched();
    assert!(d.done(checked("w1:p2", a), t).is_empty());
}

#[test]
fn reports_route_to_the_model_and_the_notices() {
    let (mut d, t) = driver();
    let nav = d.launched();
    let gone = Done::Checked {
        pane: "w1:p4".into(),
        label: "Tab 4".into(),
        keep_focus: false,
        seen: vec![],
        exists: false,
        nav,
    };
    assert!(
        matches!(&d.done(gone, t)[..], [Effect::Toast(n)] if n.text == "Tab 4 is gone" && n.seconds == 3)
    );
    let n = Toast::new(Some("x"), "y", 8);
    assert!(matches!(&d.done(Done::Toast(n.clone()), t)[..], [Effect::Toast(m)] if *m == n));
    d.panel.switch("w1:p4".into(), true);
    let out = d.done(
        Done::Switched(Switched {
            err: Some("boom".into()),
            took_back: false,
        }),
        t,
    );
    assert!(
        out.iter()
            .any(|e| matches!(e, Effect::Toast(n) if n.warning))
    );
    assert!(
        !d.panel.switch("w1:p5".into(), true).is_empty(),
        "the switch still counts as running"
    );
}
