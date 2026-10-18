use super::*;
use crate::view::Unseen;

fn swarm(orch: serde_json::Value) -> Swarm {
    let mut o =
        serde_json::json!({"pane_id": "p", "tab_id": null, "tab_label": "x", "session_id": null});
    o.as_object_mut()
        .unwrap()
        .extend(orch.as_object().unwrap().clone());
    serde_json::from_value(serde_json::json!({ "orchestrators": [o] })).unwrap()
}

fn with_agent(status: &str) -> Swarm {
    swarm(
        serde_json::json!({"threads": [{"id": "t", "name": "n", "source": "turn",
        "agents": [{"id": "a", "status": status}]}]}),
    )
}

#[test]
fn still_unless_something_moves() {
    let none = Marks::new();
    assert!(!animated(
        &swarm(serde_json::json!({"herdr_status": "idle"})),
        &none
    ));
    assert!(!animated(&with_agent("done"), &none));
    assert!(!animated(&with_agent("rev"), &none));
    for st in ["run", "fail", "blk", "wait", "bg"] {
        assert!(animated(&with_agent(st), &none), "{st}");
    }
    assert!(animated(
        &swarm(serde_json::json!({"herdr_status": "working"})),
        &none
    ));
    assert!(animated(&swarm(serde_json::json!({"inbox": [1]})), &none));
    // tests/test_states.py::test_the_clock_turns_once_a_second_and_keeps_frames_running: an
    // orchestrator whose only motion is its background tasks' waiting clock keeps frames
    // running; an idle one with old activity does not
    assert!(animated(
        &swarm(
            serde_json::json!({"bg": [{"id": "b", "kind": "Bash", "description": "w", "started": 0.0}]})
        ),
        &none
    ));
    assert!(!animated(
        &swarm(serde_json::json!({"herdr_status": "idle", "last_activity": 0.0})),
        &none
    ));
    let marks: Marks = [("a".to_owned(), Unseen::Done)].into();
    assert!(animated(&with_agent("done"), &marks));
}
