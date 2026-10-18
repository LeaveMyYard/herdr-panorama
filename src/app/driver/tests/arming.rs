//! tests/test_mouse.py's arrival rule: a cell whose click target changed in a refresh ignores
//! clicks for a second; animation frames and the hover stamp nothing.

use super::*;
use panorama_core::model::{Orchestrator, Status};
use panorama_core::view::target::AgentRef;

const WIDE: (i32, i32) = (160, 45);

#[test]
fn first_paint_targets_ignore_clicks_for_a_second() {
    let mut r = Run::demo("w1:p1", WIDE);
    r.d.app.armed_at = r.t0;
    let soon = r.t0 + Duration::from_millis(500);
    r.click(&orch("w1:p2"), soon); // just appeared
    assert_eq!(r.d.app.view.orch.as_deref(), Some("w1:p1"));
    let later = r.settled();
    r.click(&orch("w1:p2"), later);
    assert_eq!(r.d.app.view.orch.as_deref(), Some("w1:p2"));
}

#[test]
fn a_card_that_moved_in_a_refresh_ignores_clicks() {
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    let at = r.spot(&orch("w1:p3")).unwrap();
    let mut sw = wire("demo").0;
    let new_one: Orchestrator = serde_json::from_value(serde_json::json!({
        "pane_id": "w1:p0", "tab_id": "w1:t0", "tab_label": "New one", "session_id": "s0",
        "herdr_status": "idle",
    }))
    .unwrap();
    sw.orchestrators.insert(0, new_one);
    r.refresh(sw.clone(), now); // every card moved down a slot; the selected one is the same
    assert!(r.d.app.armed(now));
    let s = r.d.shown().unwrap();
    assert_eq!(s.target_at(at.0, at.1), Some(&orch("w1:p2"))); // another card is drawn there
    r.click_at(at, now + Duration::from_millis(100));
    assert_eq!((r.sel().0, r.sel().1), (0, Some("w1:p1"))); // ignored: it just arrived
    r.refresh(sw, now + Duration::from_millis(200)); // a refresh where nothing moves
    let later = now + Duration::from_secs(2);
    let out = r.click_at(at, later);
    assert_eq!(r.d.app.view.orch.as_deref(), Some("w1:p2")); // the identity drawn there
    assert!(!goes_to(&out, "w1:p2"));
}

#[test]
fn animation_frames_do_not_count_as_arrivals() {
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    for _ in 0..5 {
        r.paint(Painted::Other, now); // a frame
    }
    let (x, y) = r.spot(&orch("w1:p2")).unwrap();
    r.d.pointer(Pointer::Move, x, y, now); // a hover
    r.paint(Painted::Other, now);
    r.click(&orch("w1:p2"), now);
    assert_eq!(r.d.app.view.orch.as_deref(), Some("w1:p2"));
}

#[test]
fn pods_that_shifted_ignore_clicks_even_if_their_agent_card_did_not_move() {
    // test_mouse.py::test_r7_m1_pods_that_shifted_ignore_clicks_even_if_their_agent_card_did_not_move
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    let before = format!("{:?}", r.sel());
    assert_eq!(
        r.sel(),
        (0, Some("w1:p1"), Some("ref:#42"), Some("a1c3d90"))
    );
    let s = r.d.shown().unwrap().clone();
    let pods: Vec<((i32, i32), Target)> = s
        .targets
        .iter()
        .filter(|h| {
            h.x1 - h.x0 == 3 && matches!(&h.target, Target::Agent(a) if a.thread == "ref:#42")
        })
        .map(|h| ((h.x0, h.y), h.target.clone()))
        .collect();
    let mut sw = wire("demo").0;
    let a3 = sw.orchestrators[0].threads[0]
        .agents
        .iter_mut()
        .find(|a| a.id == "a3e1b22")
        .unwrap();
    a3.status = Status::Failed; // the failed agent sorts first: every pod shifts a slot
    r.refresh(sw, now);
    let s = r.d.shown().unwrap().clone();
    let shifted: Vec<(i32, i32)> = pods
        .iter()
        .filter(|(at, t)| s.target_at(at.0, at.1) != Some(t))
        .map(|(at, _)| *at)
        .collect();
    let a1 = Target::Agent(AgentRef {
        pane: "w1:p1".into(),
        thread: "ref:#42".into(),
        agent: "a1c3d90".into(),
    });
    assert!(shifted.iter().any(|&(x, y)| s.target_at(x, y) == Some(&a1)));
    for at in shifted {
        let out = r.click_at(at, now + Duration::from_millis(100));
        assert_eq!(format!("{:?}", r.sel()), before);
        assert!(!goes_to(&out, "w1:p1"));
    }
}
