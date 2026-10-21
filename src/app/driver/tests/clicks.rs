//! tests/test_mouse.py: clicks on cards, filters, the + row and action labels (wide).

use super::*;
use panorama_core::view::Filter;
use panorama_core::view::nav::nav;
use panorama_core::view::state::{DONE_CARD, FIN_CARD, NEW_ROW};
use panorama_core::view::target::AgentRef;

const WIDE: (i32, i32) = (160, 45);

fn agent(pane: &str, thread: &str, agent: &str) -> AgentRef {
    AgentRef {
        pane: pane.into(),
        thread: thread.into(),
        agent: agent.into(),
    }
}

fn thread(pane: &str, thread: &str) -> Target {
    Target::Thread {
        pane: pane.into(),
        thread: thread.into(),
    }
}

#[test]
fn wide_cards_select_and_a_second_click_goes_to_the_pane() {
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    r.click(&orch("w1:p2"), now);
    assert_eq!(
        r.sel(),
        (0, Some("w1:p2"), Some("wf:wf_search"), Some("j1a2b3c"))
    );
    r.click(&orch("w1:p1"), now);
    assert_eq!(
        r.sel(),
        (0, Some("w1:p1"), Some("ref:#42"), Some("a1c3d90"))
    );
    r.click(&thread("w1:p1", "ref:#44"), now);
    assert_eq!(
        r.sel(),
        (1, Some("w1:p1"), Some("ref:#44"), Some("b3a9c41"))
    );
    let out = r.click(&thread("w1:p1", "ref:#44"), now); // a thread has no pane
    assert_eq!(
        r.sel(),
        (1, Some("w1:p1"), Some("ref:#44"), Some("b3a9c41"))
    );
    assert!(!goes_to(&out, "w1:p1"));
    let b3 = Target::Agent(agent("w1:p1", "ref:#44", "b3a9c41"));
    let out = r.click(&b3, now);
    assert_eq!(
        r.sel(),
        (2, Some("w1:p1"), Some("ref:#44"), Some("b3a9c41"))
    );
    assert!(!goes_to(&out, "w1:p1")); // selecting never acts
    let out = r.click(&b3, now); // selected: g
    assert!(goes_to(&out, "w1:p1"));
}

#[test]
fn a_selected_orchestrator_clicked_again_goes_to_its_pane() {
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    assert!(goes_to(&r.click(&orch("w1:p1"), now), "w1:p1"));
}

#[test]
fn a_selected_card_without_focus_is_focused_first() {
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    r.key(Key::Right, now); // the threads; the orchestrator stays selected
    let out = r.click(&orch("w1:p1"), now);
    assert_eq!(r.sel().0, 0);
    assert!(out.is_empty());
}

#[test]
fn pods_in_a_thread_card_select_their_agent() {
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    let s = r.d.shown().unwrap();
    let pod = s
        .targets
        .iter()
        .find(|h| {
            h.x1 - h.x0 == 3 && matches!(&h.target, Target::Agent(a) if a.thread == "ref:#44")
        })
        .unwrap()
        .clone();
    let Target::Agent(a) = &pod.target else {
        unreachable!()
    };
    let id = a.agent.clone();
    r.click_at((pod.x0 + 1, pod.y), now);
    assert_eq!(
        r.sel(),
        (2, Some("w1:p1"), Some("ref:#44"), Some(id.as_str()))
    );
}

#[test]
fn complete_and_finished_cards_fold_like_enter() {
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    let done = Target::Done("w1:p1".into());
    r.click(&done, now);
    assert_eq!(
        r.d.app.view.thread.as_deref(),
        Some(format!("{DONE_CARD}w1:p1").as_str())
    );
    assert!(r.d.app.view.done_open.contains("w1:p1"));
    assert!(
        nav(&r.d.app.swarm, &r.d.app.view)[1]
            .iter()
            .any(|t| t == "ref:#40")
    );
    r.click(&done, now);
    assert!(!r.d.app.view.done_open.contains("w1:p1"));
    r.click(&thread("w1:p1", "ref:#44"), now);
    let fin = Target::Fin {
        pane: "w1:p1".into(),
        thread: "ref:#44".into(),
        group: None,
    };
    r.click(&fin, now);
    assert_eq!(
        r.d.app.view.agent.as_deref(),
        Some(format!("{FIN_CARD}ref:#44").as_str())
    );
    assert!(r.d.app.view.fin_open.contains("ref:#44") && r.sel().0 == 2);
    r.click(&fin, now);
    assert!(!r.d.app.view.fin_open.contains("ref:#44"));
}

#[test]
fn header_filters_switch_on_click() {
    let mut r = Run::demo("w1:p2", WIDE);
    let now = r.settled();
    r.click(&Target::Filter(Filter::Needs), now);
    assert_eq!(
        (r.d.app.view.filter, r.d.app.view.orch.as_deref()),
        (Filter::Needs, Some("w1:p1"))
    );
    r.click(&Target::Filter(Filter::Running), now);
    assert_eq!(r.d.app.view.filter, Filter::Running);
    r.click(&Target::Filter(Filter::All), now);
    assert_eq!(r.d.app.view.filter, Filter::All);
}

#[test]
fn the_new_agent_row_opens_the_form_only_when_armed() {
    let mut r = Run::new(wire("demo"), "w1:p1", true, WIDE);
    let now = r.settled();
    r.d.app.armed_at = now + Duration::from_secs(30); // right after a background change
    assert!(r.click(&Target::NewAgent, now).is_empty());
    r.d.app.armed_at = r.t0;
    let out = r.click(&Target::NewAgent, now);
    assert!(
        matches!(out.as_slice(), [Effect::NewAgent { .. }]),
        "{out:?}"
    );
    assert_eq!(r.d.app.view.orch.as_deref(), Some(NEW_ROW));
}

#[test]
fn action_labels_act_on_their_agent() {
    let mut r = Run::new(wire("demo"), "w1:p1", true, WIDE);
    let now = r.settled();
    r.key(Key::Right, now);
    r.key(Key::Right, now);
    let a = agent("w1:p1", "ref:#42", "a1c3d90");
    let out = r.click(&Target::Transcript(a.clone()), now);
    assert!(
        out.iter()
            .any(|e| matches!(e, Effect::Toast(t) if t.title.as_deref() == Some("transcript")))
    );
    r.d.app.armed_at = now + Duration::from_secs(30);
    assert!(!goes_to(&r.click(&Target::Goto(a.clone()), now), "w1:p1")); // not armed, like g
    r.d.app.armed_at = r.t0;
    assert!(goes_to(&r.click(&Target::Goto(a), now), "w1:p1"));
}

#[test]
fn a_click_on_empty_space_does_nothing() {
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    let before = (format!("{:?}", r.sel()), r.d.app.view.filter);
    let s = r.d.shown().unwrap().clone();
    let empty: Vec<(i32, i32)> = (0..s.h())
        .flat_map(|y| (0..s.w()).map(move |x| (x, y)))
        .filter(|&(x, y)| s.target_at(x, y).is_none())
        .collect();
    for at in [empty[0], empty[empty.len() / 2], empty[empty.len() - 1]] {
        assert!(r.click_at(at, now).is_empty());
    }
    assert_eq!((format!("{:?}", r.sel()), r.d.app.view.filter), before);
}

#[test]
fn a_vanished_identity_is_not_acted_on() {
    use crate::ui::input::Clicks;
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    let before = format!("{:?}", r.sel());
    // the model swapped, the screen not yet repainted: the stale card must not act
    r.d.app
        .graph
        .swarm
        .orchestrators
        .retain(|o| o.pane_id != "w1:p2");
    let gone = agent("w1:p1", "ref:#42", "gone");
    let mut out = vec![];
    for t in [
        orch("w1:p2"),
        thread("w1:p2", "wf:wf_search"),
        Target::Agent(gone.clone()),
        Target::Goto(gone.clone()),
        Target::Answer(gone.clone()),
        Target::Transcript(gone),
        Target::Fin {
            pane: "w1:p1".into(),
            thread: "ref:#nope".into(),
            group: None,
        },
        Target::Done("w1:p2".into()),
    ] {
        out.extend(r.d.app.click_target(t, now));
    }
    assert_eq!(format!("{:?}", r.sel()), before);
    assert!(r.d.app.view.done_open.is_empty() && r.d.app.view.fin_open.is_empty());
    assert!(out.is_empty(), "{out:?}");
}

#[test]
fn fold_clicks_are_armed_like_enter() {
    // test_mouse.py::test_r7_l1_fold_clicks_are_armed_like_enter
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    r.d.app.armed_at = now + Duration::from_secs(30);
    r.click(&Target::Done("w1:p1".into()), now);
    assert!(r.d.app.view.done_open.is_empty());
    r.d.app.armed_at = r.t0;
    r.click(&Target::Done("w1:p1".into()), now);
    assert!(r.d.app.view.done_open.contains("w1:p1"));
}

#[test]
fn hover_highlights_one_card_and_repaints_only_on_change() {
    use panorama_core::view::palette::{FAINT, LINE};
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    let (x, y) = r.spot(&orch("w1:p2")).unwrap();
    let s = r.d.shown().unwrap();
    let card: Vec<_> = s
        .targets
        .iter()
        .filter(|h| h.target == orch("w1:p2"))
        .collect();
    let (x0, top) = (card[0].x0, card.iter().map(|h| h.y).min().unwrap());
    assert_eq!(s.cell(x0, top).fg, LINE);
    let moves = [(x, y), (x + 1, y), (x + 2, y + 1)];
    let repaints: Vec<_> = moves
        .iter()
        .map(|&(x, y)| r.d.pointer(Pointer::Move, x, y, now).1)
        .collect();
    assert_eq!(repaints, [Some(Painted::Other), None, None]); // the same card: no repaint
    r.paint(Painted::Other, now);
    assert_eq!(r.d.app.view.hover, Some(orch("w1:p2")));
    assert_eq!(r.d.shown().unwrap().cell(x0, top).fg, FAINT); // the hover border
    assert_eq!(
        r.d.pointer(Pointer::Move, 80, 0, now).1,
        Some(Painted::Other)
    ); // the header
    assert_eq!(r.d.app.view.hover, None);
}
