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
