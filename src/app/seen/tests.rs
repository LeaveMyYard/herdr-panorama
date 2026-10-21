//! Agents seen in the app reach the unseen state file, as Python `_seen` writes them.

use super::*;
use crate::unseen::Event;
use panorama_core::model::{Agent, Orchestrator, Status, Swarm, Thread};
use serde_json::json;

fn swarm(status: &str) -> Swarm {
    let thread: Thread =
        serde_json::from_value(json!({"id": "t1", "name": "t", "source": "turn"})).unwrap();
    let mut o: Orchestrator = serde_json::from_value(
        json!({"pane_id": "w1:p1", "tab_id": "t", "tab_label": "o", "session_id": "s"}),
    )
    .unwrap();
    let agent = |id: &str| Agent {
        id: id.into(),
        status: Status::from_code(status),
        ..Agent::default()
    };
    o.threads = vec![Thread {
        agents: vec![agent("a"), agent("b")],
        ..thread
    }];
    Swarm {
        orchestrators: vec![o],
        ..Swarm::default()
    }
}

#[test]
fn seen_agents_clear_here_and_in_the_state_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("unseen.json");
    let t = Arc::new(Tracker::new(path.clone(), Box::new(|| true)));
    t.update(&swarm("run"), 1.0);
    assert_eq!(t.update(&swarm("done"), 2.0).len(), 2);
    assert!(mark(&t, &["x".into()]).is_none()); // no mark: nothing to save
    mark(&t, &["a".into()]).unwrap().join().unwrap();
    let only_b = [("b".to_owned(), Event::Done)].into();
    assert_eq!(t.marks(), only_b);
    // another panorama reading the file sees it seen too
    let other = Tracker::new(path, Box::new(|| true));
    assert_eq!(other.update(&swarm("done"), 3.0), only_b);
}
