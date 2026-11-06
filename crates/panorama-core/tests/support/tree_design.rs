//! tests/design_deep.py `deep_swarm` and tests/design_wide.py `wide_swarm` (the same code over
//! turn 5's and turn 7's datasets), ported: one root holding the design's agent tree.

use panorama_core::model::Swarm;
use serde_json::{Value, json};

/// Their `NOW`.
pub const NOW: f64 = 1_800_000_000.0;

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
        "parent": parent, "depth": depth,
    }));
    for k in a["kids"].as_array().unwrap() {
        add(k, a["id"].as_str(), depth + 1, qid, out);
    }
}

/// The swarm of a design dataset's `tree` (design_5d.json, design_7.json; its `thread` is
/// left out: the uniform tree has none).
pub fn tree_swarm(data: &Value) -> Swarm {
    let (mut agents, mut qid) = (vec![], 0);
    for a in data["tree"].as_array().unwrap() {
        add(a, None, 1, &mut qid, &mut agents);
    }
    let inbox: Vec<i64> = (1..=qid).collect();
    serde_json::from_value(json!({
        "generated_at": NOW, "herdr_ok": true, "inbox_ok": true,
        "roots": [{
            "pane_id": "w1:p3", "tab_id": "w1:t3", "tab_label": "Core Library", "session_id": null,
            "herdr_status": "working", "tab_no": 3, "inbox": inbox,
            "agents": agents,
        }],
    }))
    .unwrap()
}
