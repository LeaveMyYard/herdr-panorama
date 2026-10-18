//! When keys and clicks act: every arming path, at its boundary (Python `ARM_SECONDS` 0.4 s
//! after what a key acts on changed).

use crate::panel::input::fixtures::{frame, loaded_at, panel, state};
use crate::panel::input::{ARM, Effect, Key, Painted};
use panorama_core::view::target::AgentRef;
use panorama_core::view::{Screen, Target};
use serde_json::json;
use std::time::Duration;

const TICK: Duration = Duration::from_millis(1);

#[test]
fn the_first_state_arms_the_keys_for_400_ms() {
    let (mut p, t0) = loaded_at("w1:t2", "w1:t2");
    assert!(p.on_key(Key::Goto, t0 + ARM - TICK).is_empty());
    assert!(!p.on_key(Key::Goto, t0 + ARM).is_empty());
}

#[test]
fn a_refresh_that_changes_the_selected_card_re_arms_them_and_one_that_does_not_does_not() {
    let (mut p, t) = panel("w1:t2", "w1:t2");
    // the same state again: nothing changed under the keys
    p.on_state(
        state(json!({"visible_tabs": ["w1:t2"], "focused_pane": "w1:p90"})),
        t,
    );
    assert!(!p.on_key(Key::Goto, t + TICK).is_empty());
    // the selected orchestrator (Data Pipeline) changes its state: keys wait 400 ms
    let mut st = state(json!({"visible_tabs": ["w1:t2"], "focused_pane": "w1:p90"}));
    st.swarm.orchestrators[1].herdr_status = panorama_core::model::HerdrStatus::Working;
    p.on_state(st, t);
    assert!(p.on_key(Key::Goto, t + ARM - TICK).is_empty());
    assert!(!p.on_key(Key::Goto, t + ARM).is_empty());
}

#[test]
fn a_refresh_that_docks_the_panel_re_arms_the_keys() {
    // the reviewer's probe (#73): a narrow panel whose tab gets an orchestrator
    let (mut p, t) = panel("w1:t9", "w1:t1");
    assert!(!p.input.dock);
    p.on_key(Key::Right, t);
    p.on_key(Key::Right, t);
    let mut st = state(json!({"visible_tabs": ["w1:t1"], "focused_pane": "w1:p90"}));
    st.swarm.orchestrators[0].tab_id = Some("w1:t9".into());
    let t1 = t + Duration::from_secs(5);
    p.on_state(st, t1);
    assert!(p.input.dock && p.view.focus == 0);
    p.paint(46, 45, &frame());
    assert!(
        p.on_key(Key::Answer, t1 + Duration::from_millis(10))
            .is_empty()
    );
}

/// A screen with just `target` at (0, 0).
fn screen_with(target: Target) -> Screen {
    let mut s = Screen::new(46, 45);
    s.target(0, 1, 0, target);
    s
}

#[test]
fn armed_clicks_wait_like_their_keys() {
    let agent = |p: &crate::panel::model::Panel| {
        let v = panorama_core::view::View {
            orch: Some("w1:p1".into()),
            ..Default::default()
        };
        let thread = panorama_core::view::nav::nav(&p.swarm, &v)[1][0].clone();
        let v = panorama_core::view::View {
            thread: Some(thread.clone()),
            ..v
        };
        let agent = panorama_core::view::nav::nav(&p.swarm, &v)[2][0].clone();
        AgentRef {
            pane: "w1:p1".into(),
            thread,
            agent,
        }
    };
    let acts = |t: Target, check: fn(&[Effect]) -> bool| {
        let (mut p, t0) = loaded_at("w1:t9", "w1:t1");
        let s = screen_with(t);
        let cursor = |p: &crate::panel::model::Panel| {
            (
                p.view.orch.clone(),
                p.view.thread.clone(),
                p.view.agent.clone(),
                p.view.focus,
            )
        };
        let before = cursor(&p);
        let early = p.click(0, 0, &s, t0 + ARM - TICK);
        assert!(!check(&early), "acted unarmed: {early:?}");
        assert_eq!(cursor(&p), before, "an unarmed click moved the cursor");
        let late = p.click(0, 0, &s, t0 + ARM);
        assert!(check(&late), "did not act: {late:?}");
    };
    acts(Target::NewAgent, |o| matches!(o, [Effect::NewAgent { .. }]));
    acts(Target::AnswerOrch("w1:p1".into()), |o| {
        matches!(o, [Effect::OpenInbox { .. }])
    });
    let (p, _) = loaded_at("w1:t9", "w1:t1");
    let r = agent(&p);
    acts(Target::Goto(r.clone()), |o| {
        o.iter().any(|e| matches!(e, Effect::Goto { .. }))
    });
    acts(Target::Transcript(r.clone()), |o| {
        o.iter().any(|e| matches!(e, Effect::Toast(_)))
    });
    acts(Target::Answer(r), |o| {
        o.iter()
            .any(|e| matches!(e, Effect::OpenInbox { .. } | Effect::Goto { .. }))
    });
    // the done card: unfolding is what Enter does, armed like Enter
    let (mut p, t0) = loaded_at("w1:t9", "w1:t1");
    let s = screen_with(Target::Done("w1:p5".into()));
    p.click(0, 0, &s, t0 + ARM - TICK);
    assert!(!p.view.done_open.contains("w1:p5"));
    p.click(0, 0, &s, t0 + ARM);
    assert!(p.view.done_open.contains("w1:p5"));
}

#[test]
fn a_click_on_an_entry_the_filter_now_hides_does_nothing() {
    let (mut p, t) = panel("w1:t2", "w1:t2");
    let s = p.paint(46, 45, &frame());
    let hit = s
        .targets
        .iter()
        .find(|h| h.target == Target::Orch("w1:p5".into()))
        .unwrap()
        .clone();
    p.on_key(Key::Filter, t); // running: Docs Site (all done) is hidden now
    p.on_key(Key::Filter, t); // needs me: still hidden
    assert!(p.click(hit.x0, hit.y, &s, t).is_empty());
    assert_ne!(p.view.orch.as_deref(), Some("w1:p5"));
}

#[test]
fn click_arming_follows_refreshes_and_the_wheel_only() {
    let (mut p, t) = panel("w1:t2", "w1:t2");
    let blank = Screen::new(46, 45);
    let s = p.paint(46, 45, &frame());
    let hit = s
        .targets
        .iter()
        .find(|h| h.target == Target::Orch("w1:p4".into()))
        .unwrap()
        .clone();
    p.painted(Some(&blank), &s, Painted::Other, t);
    assert!(!p.click(hit.x0, hit.y, &s, t).is_empty()); // a frame stamps nothing
    p.painted(Some(&blank), &s, Painted::Refresh, t);
    assert!(
        p.click(hit.x0, hit.y, &s, t + Duration::from_millis(999))
            .is_empty()
    );
    let (mut p, t) = panel("w1:t2", "w1:t2");
    p.painted(Some(&blank), &s, Painted::Wheel, t);
    assert!(
        p.click(hit.x0, hit.y, &s, t + Duration::from_millis(299))
            .is_empty()
    );
    assert!(
        !p.click(hit.x0, hit.y, &s, t + Duration::from_millis(300))
            .is_empty()
    );
    p.painted(None, &s, Painted::Refresh, t); // the first paint: nothing to compare
}
