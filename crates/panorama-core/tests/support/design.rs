//! tests/design_dock.py `design_swarm`, ported: the design's sample dataset (captured from
//! docs/design/swarm-tui.js) as a model swarm.

use panorama_core::model::Swarm;
use serde_json::{Value, json};

/// The dataset `data` (a list of the design's orchestrators) as a swarm.
pub fn design_swarm(data: &Value) -> Swarm {
    let mut qid = 0;
    let mut orchs = vec![];
    for o in data.as_array().unwrap() {
        let tab = o["tab"].as_i64().unwrap();
        let (mut inbox, mut blocked, mut agents): (Vec<i64>, bool, Vec<Value>) =
            (vec![], false, vec![]);
        // the design groups a root's agents in threads; the uniform tree has them all
        for t in o["threads"].as_array().unwrap() {
            for a in t["agents"].as_array().unwrap() {
                let n = a["q"].as_i64().unwrap();
                let ids: Vec<i64> = (qid + 1..=qid + n).collect();
                qid += n;
                inbox.extend(&ids);
                blocked |= a["st"] == "blk";
                let pending: Vec<String> = a["perm"]
                    .as_str()
                    .map(|p| format!("wants {p}"))
                    .into_iter()
                    .collect();
                agents.push(json!({
                    "id": a["id"], "role": a["role"], "model": a["model"], "status": a["st"],
                    "seq": a["n"], "question_ids": ids,
                    "activity": a["act"].as_str().unwrap_or(""),
                    "last_message": a["msg"].as_str().unwrap_or(""),
                    "questions": a["qs"].as_array().cloned().unwrap_or_default(),
                    "error": a["err"], "pending": pending,
                }));
            }
        }
        orchs.push(json!({
            "pane_id": format!("w1:p{tab}"), "tab_id": format!("w1:t{tab}"), "tab_label": o["name"],
            "session_id": null, "herdr_status": if blocked { "blocked" } else { "idle" },
            "tab_no": tab, "inbox": inbox, "agents": agents,
        }));
    }
    let mut sw: Swarm =
        serde_json::from_value(json!({"herdr_ok": true, "inbox_ok": true, "roots": orchs}))
            .unwrap();
    sw.docked = Some("w1:p3".into());
    sw
}
