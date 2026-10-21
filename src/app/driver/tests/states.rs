//! tests/test_states.py's key case in the app: an orchestrator with only its main session's
//! background tasks shows a background card, reached with → ; Enter there does nothing else.

use super::*;
use panorama_core::model::{BgTask, Orchestrator};
use panorama_core::view::state::BG_CARD;

#[test]
fn the_background_card_is_reached_with_right_and_enter_does_nothing_else() {
    let (mut sw, now_s) = wire("demo");
    let mut o: Orchestrator = serde_json::from_value(serde_json::json!({
        "pane_id": "w1:p9", "tab_id": "w1:t9", "tab_label": "Nightly", "session_id": null,
        "herdr_status": "idle",
    }))
    .unwrap();
    for (i, desc) in ["cargo watch", "npm run dev"].into_iter().enumerate() {
        o.bg.push(BgTask {
            id: format!("b{i}"),
            kind: "bash".into(),
            description: desc.into(),
            started: Some(now_s - 600.0),
        });
    }
    sw.orchestrators.push(o);
    let mut r = Run::new((sw, now_s), "w1:p9", false, (160, 56));
    let now = r.settled();
    r.d.app.view.orch = Some("w1:p9".into());
    r.key(Key::Right, now);
    let card = format!("{BG_CARD}w1:p9");
    assert_eq!(
        (r.d.app.view.focus, r.d.app.view.thread.as_deref()),
        (1, Some(card.as_str()))
    );
    let out = r.key(Key::Enter, now);
    assert!(out.is_empty(), "{out:?}");
    assert_eq!(r.d.app.view.focus, 1); // no agents behind it: Enter stays
    assert!(
        r.text().contains("background  2 tasks · main session"),
        "{}",
        r.text()
    );
}
