//! tests/test_inbox_link.py's clicks in the app: an agent card's answer label deep-links its
//! question, an orchestrator's ?N opens its pane's group, armed and only where nothing just
//! moved. The inbox opening (after a popup closes, over the home view) is the link's
//! (`link/tests/acts.rs`).

use super::*;
use panorama_core::model::Orchestrator;
use panorama_core::view::target::AgentRef;

const WIDE: (i32, i32) = (160, 45);

#[test]
fn a_click_on_an_agent_cards_answer_deep_links_its_question() {
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    r.key(Key::Right, now);
    r.key(Key::Move(1), now);
    r.key(Key::Move(1), now);
    r.key(Key::Right, now); // PR #48: the asking agent, its card expanded
    let answer = Target::Answer(AgentRef {
        pane: "w1:p1".into(),
        thread: "ref:#48".into(),
        agent: "c1d2e3f".into(),
    });
    assert!(r.spot(&answer).is_some());
    let later = now + Duration::from_secs(2); // the first paint, a while later
    r.d.app.armed_at = later + Duration::from_secs(30); // as right after a background change
    let out = r.click(&answer, later); // not armed: no inbox (the card is looked at: seen)
    assert_eq!(out, [Effect::Seen(vec!["c1d2e3f".into()])]);
    r.d.app.armed_at = r.t0;
    let out = r.click(&answer, later);
    let inbox = Effect::OpenInbox {
        question: Some(2),
        pane: None,
    };
    assert!(out.contains(&inbox), "{out:?}");
}

#[test]
fn a_click_on_an_orchestrators_mark_opens_its_pane_group() {
    let mut r = Run::new(wire("demo"), "w1:p1", true, WIDE);
    let now = r.settled() + Duration::from_secs(2);
    let out = r.click(&Target::AnswerOrch("w1:p1".into()), now);
    assert_eq!(
        out,
        [Effect::OpenInbox {
            question: None,
            pane: Some("w1:p1".into())
        }]
    );
}

#[test]
fn a_mark_that_moved_in_the_last_refresh_ignores_clicks() {
    let mut r = Run::new(wire("demo"), "w1:p1", true, WIDE);
    let now = r.settled();
    r.refresh(wire("demo").0, now); // a refresh where nothing moved
    let mark = Target::AnswerOrch("w1:p1".into());
    let (_, y) = r.spot(&mark).unwrap();
    let mut sw = wire("demo").0;
    let new_one: Orchestrator = serde_json::from_value(serde_json::json!({
        "pane_id": "w1:p0", "tab_id": "w1:t0", "tab_label": "New one", "session_id": "s0",
        "herdr_status": "idle",
    }))
    .unwrap();
    sw.orchestrators.insert(0, new_one);
    r.refresh(sw, now); // the selected card is the same (no re-arm), every ?N moved down
    assert!(r.d.app.armed(now));
    let moved = r.spot(&mark).unwrap();
    assert_eq!(moved.1, y + 5);
    assert!(
        r.click_at(moved, now + Duration::from_millis(100))
            .is_empty()
    ); // just moved
    let out = r.click_at(moved, now + Duration::from_secs(2)); // a second later
    assert_eq!(
        out,
        [Effect::OpenInbox {
            question: None,
            pane: Some("w1:p1".into())
        }]
    );
}
