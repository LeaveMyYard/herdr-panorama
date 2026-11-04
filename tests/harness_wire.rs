//! The additive identity survives every Rust serialization route and old wire states.
use panorama::app::link::state::{Published, state_message};
use panorama::proto::state::{State, cleaned_swarm};
use panorama::snapshot::{Format, render};
use panorama_core::model::Swarm;
use panorama_core::view::Marks;
use serde_json::{Value, json};

#[test]
fn harness_identity_defaults_only_when_missing_and_roundtrips_every_serializer() {
    for harness in [None, Some("claude"), Some("codex"), Some("future-harness")] {
        let mut orch = json!({"pane_id":"p", "tab_id":"t", "tab_label":"work", "session_id":null});
        if let Some(h) = harness {
            orch["harness"] = json!(h);
        }
        let sw: Swarm = serde_json::from_value(json!({"roots":[orch]})).unwrap();
        let expected = harness.unwrap_or("claude");
        assert_eq!(sw.roots[0].harness, expected);
        assert_eq!(
            serde_json::to_value(&sw).unwrap()["roots"][0]["harness"],
            expected
        );
        let snapshot: Value = serde_json::from_str(&render(&sw, Format::Json, 0.0)).unwrap();
        assert_eq!(snapshot["roots"][0]["harness"], expected);
        let msg = state_message(&sw, &Marks::default(), &Published::default(), None);
        assert_eq!(msg["state"]["swarm"]["roots"][0]["harness"], expected);
        let state = State::read(msg["state"].as_object().unwrap()).unwrap();
        assert_eq!(state.swarm.roots[0].harness, expected);
        assert_eq!(cleaned_swarm(state.swarm).roots[0].harness, expected);
    }
}
