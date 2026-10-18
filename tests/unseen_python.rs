//! The unseen tracker against Python's (`tests/fixtures/unseen/steps.json`, captured by
//! `tools/capture_unseen.py` from `unseen.Tracker`): the same refreshes on one state file give
//! the same marks and the same file, byte for byte, after every step.

use panorama::unseen::{Marks, Tracker};
use panorama_core::model::{Agent, Orchestrator, Status, Swarm, Thread};
use serde_json::{Value, json};
use std::path::Path;

fn swarm(spec: &Value) -> Swarm {
    let orchestrators = spec
        .as_array()
        .unwrap()
        .iter()
        .map(|x| {
            let agents: Vec<Agent> = x["agents"]
                .as_array()
                .unwrap()
                .iter()
                .map(|g| Agent {
                    id: g["id"].as_str().unwrap().into(),
                    status: Status::from_code(g["status"].as_str().unwrap()),
                    question_ids: g["qids"].as_array().unwrap().iter().map(|q| q.as_i64().unwrap()).collect(),
                    ended: g["ended"].as_f64(),
                    last_at: g["last_at"].as_f64(),
                    started: g["started"].as_f64(),
                    ..Agent::default()
                })
                .collect();
            let thread: Thread = serde_json::from_value(json!({"id": "t1", "name": "t", "source": "turn"})).unwrap();
            let mut o: Orchestrator = serde_json::from_value(json!({
                "pane_id": x["pane"], "tab_id": "t", "tab_label": "o", "session_id": "s", "focused": x["focused"],
                "machine": x["machine"], "machine_id": x["machine_id"],
            }))
            .unwrap();
            o.threads = vec![Thread { agents, ..thread }];
            o
        })
        .collect();
    Swarm {
        orchestrators,
        ..Swarm::default()
    }
}

fn codes(m: &Marks) -> Value {
    Value::Object(
        m.iter()
            .map(|(k, e)| (k.clone(), json!(e.code())))
            .collect(),
    )
}

#[test]
fn the_tracker_writes_and_marks_as_python_s_does_step_by_step() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/unseen/steps.json");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("unseen.json");
    let fresh = || Tracker::new(file.clone(), Box::new(|| true));
    let mut tracker = fresh();
    for (i, step) in doc["steps"].as_array().unwrap().iter().enumerate() {
        if step["restart"] == true {
            tracker = fresh();
        }
        let marks = tracker.update(&swarm(&step["swarm"]), step["now"].as_f64().unwrap());
        if let Some(seen) = step["seen"].as_array() {
            let ids = tracker.seen(seen.iter().map(|s| s.as_str().unwrap()));
            tracker.save_seen(&ids);
        }
        assert_eq!(codes(&marks), step["marks"], "step {i}: marks");
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            step["file"].as_str().unwrap(),
            "step {i}: the file"
        );
    }
}
