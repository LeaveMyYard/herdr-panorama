//! tests/test_states.py's key case in the app, on the uniform tree (plan
//! `uniform-agent-tree.md` §4): a root with only its own background tasks lists them in its
//! card; with no agents below it, → and Enter stay on it.

use super::*;
use panorama_core::model::{BgTask, Orchestrator};

#[test]
fn a_roots_background_work_is_in_its_card_and_right_and_enter_stay() {
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
    assert_eq!(r.d.app.view.focus, 0); // no agents: nothing to the right
    let out = r.key(Key::Enter, now);
    assert!(out.is_empty(), "{out:?}");
    assert_eq!(r.d.app.view.focus, 0);
    let text = r.text();
    assert!(
        text.contains("cargo watch") && text.contains("npm run dev"),
        "{text}"
    );
}
