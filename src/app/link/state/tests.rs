use super::*;
use crate::proto::state::State;

fn fixture() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/inputs/swarm-design9.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn the_swarm_goes_out_as_python_sent_it() {
    // the fixture's swarm is Python's own wire form (swarm_to_wire): read and written back,
    // every field is the same
    let wire = fixture()["swarm"].clone();
    let swarm: Swarm = serde_json::from_value(wire.clone()).unwrap();
    let msg = state_message(&swarm, &Marks::new(), &Published::default(), None);
    assert_eq!(msg["state"]["swarm"], wire);
}

#[test]
fn the_state_carries_focus_tab_panels_and_filter_and_the_panel_reads_it() {
    let swarm: Swarm = serde_json::from_value(fixture()["swarm"].clone()).unwrap();
    let unseen: Marks = [("a5".to_string(), panorama_core::view::Unseen::Done)].into();
    let p = Published {
        focused_pane: Some("w1:p3".into()),
        visible_tab: Some("w1:t3".into()),
        panels: [("w1:t2".to_string(), "w1:p92".to_string())].into(),
    };
    let msg = state_message(&swarm, &unseen, &p, Some("needs"));
    let st = &msg["state"];
    assert_eq!(st["protocol"], crate::proto::PROTOCOL);
    assert_eq!(st["unseen"], json!({"a5": "done"}));
    assert_eq!(st["focused_pane"], "w1:p3");
    assert_eq!(st["visible_tabs"], json!(["w1:t3"]));
    assert_eq!(st["panels"], json!({"w1:t2": "w1:p92"}));
    assert_eq!(st["filter"], "needs");
    // the panel viewer reads what the core writes
    let read = State::read(st.as_object().unwrap()).expect("a state the panel reads");
    assert_eq!(read.focused_pane.as_deref(), Some("w1:p3"));
    let none = state_message(&swarm, &unseen, &Published::default(), None);
    assert_eq!(
        none["state"]["visible_tabs"],
        Value::Null,
        "unknown: null, as Python's None"
    );
    assert_eq!(none["state"]["focused_pane"], Value::Null);
    assert!(
        none["state"].get("filter").is_none(),
        "no filter: the field is left out"
    );
}
