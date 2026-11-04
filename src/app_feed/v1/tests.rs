use super::*;
use crate::snapshot::pyjson::dumps;
use serde_json::{Value, json};

fn agent(id: &str, parent: Option<&str>, status: Status, started: f64) -> Agent {
    Agent {
        id: id.into(),
        parent: parent.map(str::to_owned),
        status,
        started: Some(started),
        ..Agent::default()
    }
}

fn wire(s: &Swarm) -> Value {
    serde_json::from_str(&dumps(&swarm(s), 0)).unwrap()
}

#[test]
fn every_root_carries_one_thread_of_all_its_agents() {
    let mut o: Root = serde_json::from_value(json!({"pane_id": "p", "tab_id": null,
        "tab_label": "t", "session_id": null, "title": "Ship it", "hidden_finished": 2}))
    .unwrap();
    let mut done = agent("a", None, Status::Done, 1.0);
    done.ended = Some(8.0);
    let mut failed = agent("b", Some("a"), Status::Failed, 2.0);
    failed.last_at = Some(9.0);
    o.agents = vec![done, failed];
    let s = Swarm {
        roots: vec![o.clone()],
        ..Swarm::default()
    };
    let w = wire(&s);
    let root = &w["orchestrators"][0];
    assert_eq!(
        (&root["flavour"], &root["hidden_done"]),
        (&json!("agents"), &json!(0))
    );
    assert!(root.get("agents").is_none() && root.get("hidden_finished").is_none());
    let [t] = root["threads"].as_array().unwrap().as_slice() else {
        panic!("one thread: {root}")
    };
    assert_eq!(
        (&t["id"], &t["name"], &t["source"], &t["phases"]),
        (
            &json!(TREE_THREAD),
            &json!("Ship it"),
            &json!("session"),
            &json!([])
        )
    );
    assert_eq!(t["agents"][1]["parent"], "a");
    assert_eq!(t["agents"][0]["phases"], json!([]));
    // the newest (by its last record) failed
    assert_eq!(t["status"], "failed");
    assert_eq!((&t["started"], &t["ended"]), (&json!(1.0), &json!(9.0)));
    assert_eq!(
        (
            &w["counts"]["orchestrators"],
            &w["counts"]["threads"],
            &w["counts"]["active_threads"]
        ),
        (&json!(1), &json!(1), &json!(1))
    );
    // a root with no agents still has its one thread, empty and done
    o.agents.clear();
    let w = wire(&Swarm {
        roots: vec![o],
        ..Swarm::default()
    });
    let t = &w["orchestrators"][0]["threads"][0];
    assert_eq!(
        (&t["agents"], &t["status"], &t["ended"]),
        (&json!([]), &json!("done"), &json!(null))
    );
    assert_eq!(w["counts"]["active_threads"], 0);
}

#[test]
fn a_live_agent_keeps_the_thread_active_and_open() {
    let agents = [agent("a", None, Status::Running, 3.0)];
    assert_eq!(status(&agents), "active");
    let cancelled = [agent("a", None, Status::Cancelled, 3.0)];
    assert_eq!(status(&cancelled), "cancelled");
}
