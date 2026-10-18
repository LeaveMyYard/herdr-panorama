use super::*;

fn orch(extra: serde_json::Value) -> Orchestrator {
    let mut v =
        serde_json::json!({"pane_id": "p", "tab_id": null, "tab_label": "x", "session_id": null});
    v.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}

fn shown(swarm: &Swarm, f: Filter) -> Vec<&str> {
    let view = View {
        filter: f,
        now: 100_000.0,
        ..View::default()
    };
    visible_orchs(swarm, &view)
        .iter()
        .map(|o| o.tab_label.as_str())
        .collect()
}

#[test]
fn running_keeps_work_in_flight_and_waiting_but_not_stale() {
    let bg = serde_json::json!([{"id": "b", "kind": "k", "started": 99_000.0}]);
    let swarm = Swarm {
        orchestrators: vec![
            orch(serde_json::json!({"tab_label": "working", "herdr_status": "working"})),
            orch(serde_json::json!({"tab_label": "waiting", "bg": bg})),
            orch(serde_json::json!({"tab_label": "idle"})),
            orch(
                serde_json::json!({"tab_label": "stale", "herdr_status": "working", "last_activity": 0.0}),
            ),
            orch(serde_json::json!({"tab_label": "asks", "inbox": [1]})),
        ],
        ..Swarm::default()
    };
    assert_eq!(shown(&swarm, Filter::All).len(), 5);
    // stale needs nothing else in flight to be stale: herdr's "working" still wins over it
    assert_eq!(
        shown(&swarm, Filter::Running),
        ["working", "waiting", "stale"]
    );
    assert_eq!(shown(&swarm, Filter::Needs), ["asks"]);
}

#[test]
fn filter_names_are_the_protocols() {
    for f in Filter::ALL {
        assert_eq!(Filter::from_name(f.name()), Some(f));
    }
    assert_eq!(Filter::from_name("nope"), None);
    assert_eq!(serde_json::to_value(Filter::Needs).unwrap(), "needs");
}
