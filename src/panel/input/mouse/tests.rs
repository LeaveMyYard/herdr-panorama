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

#[test]
fn a_click_anywhere_on_a_panel_acts_only_with_a_core_and_never_closes_it() {
    // test_panel_click.py::test_a_click_anywhere_on_a_panel_keeps_it_open[True/False]: every
    // drawn target, then empty space and a hover; with a core the clicks switch, without one
    // nothing acts
    for connected in [true, false] {
        let (mut p, mut t) = panel("w1:t2", "w1:t2");
        p.on_connected(connected);
        let s = p.paint(46, 45, &frame());
        assert!(
            s.targets
                .iter()
                .any(|h| matches!(h.target, Target::Orch(_)))
        );
        assert!(s.targets.iter().any(|h| h.target == Target::NewAgent));
        let mut acted = false;
        for hit in s.targets.clone() {
            t += Duration::from_secs(2); // armed, and no cell held
            let out = p.click(hit.x0, hit.y, &s, t);
            assert!(!out.contains(&Effect::Quit), "{:?}", hit.target);
            let moves = out.iter().any(|e| {
                matches!(
                    e,
                    Effect::Switch { .. }
                        | Effect::Goto { .. }
                        | Effect::GotoRemote { .. }
                        | Effect::OpenInbox { .. }
                        | Effect::NewAgent { .. }
                )
            });
            acted |= moves;
            assert!(
                connected || !moves,
                "{:?} acted without a core: {out:?}",
                hit.target
            );
            if out.iter().any(|e| matches!(e, Effect::Switch { .. })) {
                p.switched(None, false, t); // the switch ran (a worker in Python)
            }
        }
        assert!(p.click(1, 44, &s, t + Duration::from_secs(2)).is_empty()); // empty space
        p.hover(Some((5, 5)), &s);
        assert_eq!(acted, connected, "connected {connected}");
    }
}

#[test]
fn a_wide_panel_folds_a_finished_card_on_click_and_reports_its_agents_seen() {
    // a panel 120 columns or wider draws the wide graph (Python too): its "n finished" cards
    // fold like Enter on them, and their agents are seen, as Python's `click_target("fin")`
    let (mut p, t) = panel("w1:t9", "w1:t1");
    let s = p.paint(160, 45, &frame());
    let fin = s
        .targets
        .iter()
        .find(|h| matches!(h.target, Target::Fin { .. }))
        .expect("a finished card is drawn")
        .clone();
    let Target::Fin { thread, group, .. } = &fin.target else {
        unreachable!()
    };
    let key = group.clone().unwrap_or_else(|| thread.clone());
    let out = p.click(fin.x0 + 1, fin.y, &s, t);
    assert!(p.view.fin_open.contains(&key));
    assert!(
        matches!(&out[..], [Effect::Seen(ids)] if !ids.is_empty()),
        "{out:?}"
    );
    // armed like Enter: right after a change under the keys, a click folds nothing
    let (mut p, t) = panel("w1:t9", "w1:t1");
    let s = p.paint(160, 45, &frame());
    p.input.armed_at = t + Duration::from_secs(30);
    assert!(p.click(fin.x0 + 1, fin.y, &s, t).is_empty());
    assert!(p.view.fin_open.is_empty());
}

#[test]
fn a_click_on_a_question_mark_answers_and_does_not_switch() {
    // test_panel_keys.py::test_one_click_on_an_entry_docks_and_question_marks_answer (the
    // switch half: a_click_on_an_entry_in_the_docked_list_switches_to_it)
    let (mut p, t) = panel("w1:t2", "w1:t2");
    let s = p.paint(46, 45, &frame());
    let ask = s
        .targets
        .iter()
        .find(|h| matches!(h.target, Target::AnswerOrch(_)))
        .expect("a ?N is drawn")
        .clone();
    let Target::AnswerOrch(pane) = &ask.target else {
        unreachable!()
    };
    let out = p.click(ask.x0, ask.y, &s, t);
    assert!(
        matches!(&out[..], [Effect::OpenInbox { pane: Some(p2), .. }] if p2 == pane),
        "{out:?}"
    );
}

#[test]
fn a_click_on_the_docked_entry_goes_into_its_agent_like_enter() {
    // test_panel_keys.py::test_one_click_on_the_docked_entry_focuses_its_agent_like_enter
    let (mut p, t) = panel("w1:t2", "w1:t2");
    p.view.orch = Some("w1:p4".into()); // the cursor off the docked entry
    let s = p.paint(46, 45, &frame());
    let (x, y) = at(&s, &Target::Orch("w1:p2".into()));
    let out = p.click(x, y, &s, t);
    assert!(
        matches!(&out[..], [Effect::Goto { pane, keep_focus: false, .. }] if pane == "w1:p2"),
        "{out:?}"
    );
}

#[test]
fn the_wide_graph_selects_first_and_goes_on_the_second_click() {
    // test_panel_keys.py::test_the_2a_graph_keeps_select_first
    let (mut p, t) = panel("w1:t2", "w1:t2");
    let s = p.paint(160, 45, &frame());
    assert!(!p.input.dock);
    let (x, y) = at(&s, &Target::Orch("w1:p4".into()));
    assert!(p.click(x, y, &s, t).is_empty()); // the first click selects
    assert_eq!(p.view.orch.as_deref(), Some("w1:p4"));
    let s = p.paint(160, 45, &frame());
    let (x, y) = at(&s, &Target::Orch("w1:p4".into()));
    let out = p.click(x, y, &s, t);
    assert!(
        out.iter()
            .any(|e| matches!(e, Effect::Goto { pane, .. } if pane == "w1:p4")),
        "{out:?}"
    );
}

#[test]
fn an_unarmed_click_in_the_docked_list_may_select_but_never_switches() {
    // test_panel_keys.py::test_one_click_still_obeys_arming_and_identity
    let (mut p, t) = panel("w1:t2", "w1:t2");
    let s = p.paint(46, 45, &frame());
    let (x, y) = at(&s, &Target::Orch("w1:p4".into()));
    p.input.armed_at = t + Duration::from_secs(30); // a refresh just changed the selected entry
    let out = p.click(x, y, &s, t);
    assert!(
        !out.iter().any(|e| matches!(e, Effect::Goto { .. })),
        "{out:?}"
    );
    p.input.armed_at = t;
    p.graph.swarm.orchestrators.retain(|o| o.pane_id != "w1:p4"); // the model changed, not repainted
    assert!(p.click(x, y, &s, t).is_empty());
}
