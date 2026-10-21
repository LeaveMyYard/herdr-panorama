use crate::panel::input::fixtures::panel;
use crate::panel::input::{ARROW_REST, Effect, Key};
use std::time::Duration;

#[test]
fn the_arrows_switch_once_they_rest() {
    let (mut p, t) = panel("w1:t2", "w1:t2");
    let out = p.on_key(Key::Move(1), t);
    assert_eq!(out, [Effect::ArrowsRestAt(t + ARROW_REST)]);
    // another arrow before it rests moves the rest on
    let t2 = t + Duration::from_millis(100);
    p.on_key(Key::Move(1), t2);
    assert!(p.arrows_rested(t + ARROW_REST).is_empty());
    let out = p.arrows_rested(t2 + ARROW_REST);
    let pane = p.view.orch.clone().unwrap();
    assert!(
        matches!(out.last(), Some(Effect::Switch { pane: p2, keep_focus: true }) if *p2 == pane)
    );
    assert!(matches!(out.first(), Some(Effect::Seen(_))));
}

#[test]
fn no_switch_onto_the_docked_one_or_while_the_keyboard_is_elsewhere() {
    let (mut p, t) = panel("w1:t2", "w1:t2");
    p.on_key(Key::Move(1), t);
    p.on_key(Key::Move(-1), t); // back on the docked one (tab 2)
    assert!(p.arrows_rested(t + ARROW_REST).is_empty());
    let (mut p, t) = panel("w1:t2", "w1:t2");
    p.on_key(Key::Move(1), t);
    p.on_terminal_focus(false);
    assert!(p.arrows_rested(t + ARROW_REST).is_empty());
}

#[test]
fn arrows_that_rest_during_a_switch_are_looked_at_again_when_it_ends() {
    let (mut p, t) = panel("w1:t2", "w1:t2");
    assert!(!p.switch("w1:p4".into(), true).is_empty());
    assert!(p.switch("w1:p5".into(), true).is_empty()); // one at a time
    p.on_key(Key::Move(1), t);
    assert!(p.arrows_rested(t + ARROW_REST).is_empty());
    let out = p.switched(Some("w1:p4 is gone".into()), false, t + ARROW_REST);
    assert!(matches!(out[0], Effect::ArrowsRestAt(_)));
    assert!(
        matches!(&out[1], Effect::Toast(toast) if toast.warning && toast.title.as_deref() == Some("cockpit"))
    );
}

#[test]
fn the_rest_comes_due_at_200_ms_not_a_tick_before() {
    let (mut p, t) = panel("w1:t2", "w1:t2");
    p.on_key(Key::Move(1), t);
    assert!(
        p.arrows_rested(t + ARROW_REST - Duration::from_millis(1))
            .is_empty()
    );
    assert!(!p.arrows_rested(t + ARROW_REST).is_empty());
}

#[test]
fn g_cancels_a_pending_arrow_switch() {
    let (mut p, t) = panel("w1:t2", "w1:t2");
    p.on_key(Key::Move(1), t);
    p.on_key(Key::Goto, t);
    assert!(p.arrows_rested(t + ARROW_REST).is_empty());
}

#[test]
fn without_dock_keep_focus_the_arrows_only_move() {
    // Python's arrows_setting needs keep_focus: an arrow never moves the keyboard into an agent
    let (mut p, t) = panel("w1:t2", "w1:t2");
    p.input.settings.keep_focus = false;
    assert!(p.on_key(Key::Move(1), t).is_empty());
    assert!(p.arrows_rested(t + ARROW_REST).is_empty());
}

#[test]
fn g_during_a_switch_still_marks_the_agents_seen() {
    // Python's action_goto marks them seen before _dock returns on `docking`
    let (mut p, t) = panel("w1:t2", "w1:t2");
    p.switch("w1:p4".into(), true);
    p.view.orch = Some("w1:p3".into());
    assert!(matches!(&p.on_key(Key::Goto, t)[..], [Effect::Goto { seen, .. }] if !seen.is_empty()));
    assert!(p.switch("w1:p3".into(), false).is_empty()); // and no second switch
}

#[test]
fn a_switch_that_took_its_announcement_back_leaves_this_panel_focused() {
    // Python's worker: `herdr_focused_pane = my_pane` after a take-back; the arrows then act
    for (took_back, switches) in [(true, true), (false, false)] {
        let (mut p, t) = panel("w1:t2", "w1:t2");
        assert!(!p.switch("w1:p4".into(), true).is_empty());
        p.herdr_focus("w1:p4"); // the announcement's echo moved herdr's focus there
        p.switched(Some("w1:p4 is gone".into()), took_back, t);
        assert_eq!(p.herdr_focused_pane.as_deref() == Some("w1:p90"), took_back);
        p.on_key(Key::Move(1), t);
        let out = p.arrows_rested(t + ARROW_REST);
        assert_eq!(
            out.iter().any(|e| matches!(e, Effect::Switch { .. })),
            switches,
            "took_back {took_back}"
        );
    }
}

#[test]
fn the_arrows_do_not_switch_onto_new_agent_or_another_machine() {
    // test_panel_keys.py::test_arrows_do_not_dock_the_docked_one_new_agent_or_a_remote (the
    // docked one: no_switch_onto_the_docked_one_or_while_the_keyboard_is_elsewhere)
    let (mut p, t) = panel("w1:t2", "w1:t2");
    for _ in 0..9 {
        p.on_key(Key::Move(1), t); // down to + New agent, the last row
    }
    assert_eq!(p.view.orch.as_deref(), Some(panorama_core::view::NEW_ROW));
    assert!(p.arrows_rested(t + ARROW_REST).is_empty());
    let (mut p, t) = panel("w1:t2", "w1:t2");
    let remote = p
        .graph
        .swarm
        .orchestrators
        .iter_mut()
        .find(|o| o.pane_id == "w1:p3")
        .unwrap();
    remote.machine = Some("build-box".into()); // on another machine: Enter's hint, never a switch
    p.on_key(Key::Move(1), t);
    assert_eq!(p.view.orch.as_deref(), Some("w1:p3"));
    assert!(p.arrows_rested(t + ARROW_REST).is_empty());
}

#[test]
fn with_the_arrows_off_or_in_the_wide_graph_the_arrows_only_move() {
    // test_panel_keys.py::test_arrows_off_or_the_2a_graph_never_dock_on_arrows
    let (mut p, t) = panel("w1:t2", "w1:t2");
    p.input.settings.arrows = false;
    assert!(p.on_key(Key::Move(1), t).is_empty());
    assert!(p.arrows_rested(t + ARROW_REST).is_empty());
    let (mut p, t) = panel("w1:t2", "w1:t2");
    p.paint(160, 45, &crate::panel::input::fixtures::frame());
    assert!(!p.input.dock);
    assert!(p.on_key(Key::Move(1), t).is_empty());
    assert!(p.arrows_rested(t + ARROW_REST).is_empty());
}

#[test]
fn no_switch_while_herdr_says_the_keyboard_is_in_another_pane() {
    // test_panel_keys.py::test_an_arrow_dock_is_skipped_when_the_keyboard_left[herdr says another pane]
    let (mut p, t) = panel("w1:t2", "w1:t2");
    p.on_key(Key::Move(1), t);
    p.herdr_focused_pane = Some("w1:p1".into()); // herdr's last word: another pane
    assert!(p.arrows_rested(t + ARROW_REST).is_empty());
}

#[test]
fn an_arrow_resting_during_a_switch_runs_after_it_even_while_the_focus_looks_elsewhere() {
    // test_panel_keys.py::test_an_arrow_settling_during_a_dock_runs_after_it_even_if_focus_looks_elsewhere
    let (mut p, t) = panel("w1:t2", "w1:t2");
    assert!(!p.switch("w1:p4".into(), true).is_empty()); // a switch is running
    p.herdr_focused_pane = Some("w1:p2".into()); // mid-switch, herdr names another pane
    p.on_key(Key::Move(1), t);
    assert!(p.arrows_rested(t + ARROW_REST).is_empty() && p.input.arrow_again);
    p.herdr_focused_pane = p.my_pane.clone();
    let out = p.switched(None, false, t + ARROW_REST); // it finished
    assert!(matches!(out[..], [Effect::ArrowsRestAt(_)]) && !p.input.arrow_again);
    let pane = p.view.orch.clone().unwrap();
    let out = p.arrows_rested(t + ARROW_REST * 2);
    assert!(
        matches!(out.last(), Some(Effect::Switch { pane: p2, keep_focus: true }) if *p2 == pane),
        "{out:?}"
    );
}
