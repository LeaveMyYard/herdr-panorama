//! tests/design_deep.py `deep_swarm` and tests/design_wide.py `wide_swarm` (the same code over
//! turn 5's and turn 7's datasets), ported: one orchestrator whose thread nests the design's
//! agent tree.

use panorama_core::model::Swarm;
use serde_json::{Value, json};

/// Their `NOW`.
pub const NOW: f64 = 1_800_000_000.0;
/// Their thread's id.
pub const TID: &str = "x:deep";

fn phase_state(code: &str) -> &'static str {
    match code {
        "d" => "done",
        "n" => "now",
        "x" => "next",
        _ => "failed",
    }
}

fn minutes(age: &str) -> f64 {
    age.strip_suffix('m').map_or(0.0, |m| m.parse().unwrap())
}

fn add(a: &Value, parent: Option<&str>, depth: i64, qid: &mut i64, out: &mut Vec<Value>) {
    let n = a["q"].as_i64().unwrap();
    let ids: Vec<i64> = (*qid + 1..=*qid + n).collect();
    *qid += n;
    out.push(json!({
        "id": a["id"], "role": a["role"], "model": a["model"], "status": a["st"], "seq": a["n"],
        "started": NOW - minutes(a["age"].as_str().unwrap()) * 60.0, "tool_calls": a["tools"],
        "activity": a["act"], "questions": a["qs"], "question_ids": ids, "error": a["err"],
        "parent": parent, "depth": depth, "phases": [1],
    }));
    for k in a["kids"].as_array().unwrap() {
        add(k, a["id"].as_str(), depth + 1, qid, out);
    }
}

/// The swarm of a design dataset with a `thread` and a `tree` (design_5d.json, design_7.json).
pub fn tree_swarm(data: &Value) -> Swarm {
    let (mut agents, mut qid) = (vec![], 0);
    for a in data["tree"].as_array().unwrap() {
        add(a, None, 1, &mut qid, &mut agents);
    }
    let inbox: Vec<i64> = (1..=qid).collect();
    let phases: Vec<Value> = data["thread"]["ph"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| json!({"name": p[0], "state": phase_state(p[1].as_str().unwrap())}))
        .collect();
    serde_json::from_value(json!({
        "generated_at": NOW, "herdr_ok": true, "inbox_ok": true,
        "orchestrators": [{
            "pane_id": "w1:p3", "tab_id": "w1:t3", "tab_label": "Core Library", "session_id": null,
            "herdr_status": "working", "tab_no": 3, "inbox": inbox,
            "threads": [{
                "id": TID, "name": data["thread"]["name"], "source": "explicit", "status": "active",
                "started": NOW - 44.0 * 60.0, "phases": phases, "agents": agents,
            }],
        }],
    }))
    .unwrap()
}
