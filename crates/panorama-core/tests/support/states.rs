//! tests/test_states.py `states_swarm`, ported: the design's turn-8 dataset
//! (`design_8.json`'s data) over the sample swarm.

use super::design::design_swarm;
use super::fixture;
use panorama_core::model::{BgTask, Orchestrator, Status, Swarm};
use serde_json::Value;

/// The design's clock (test_states.py `NOW`).
pub const NOW: f64 = 1_800_000_000.0;
const NOW_MIN: i64 = 10 * 60 + 4; // the design's clock, 10:04

fn flavour(key: &str) -> &'static str {
    match key {
        "co" => "coordinator",
        "pr" => "pr-review",
        _ => "superpowers",
    }
}

/// Minutes from `hh:mm` back to the design's 10:04.
fn minutes(hhmm: &str) -> i64 {
    let (h, m) = hhmm.split_once(':').unwrap();
    (NOW_MIN - h.parse::<i64>().unwrap() * 60 - m.parse::<i64>().unwrap()).rem_euclid(1440)
}

fn bg_task(id: String, b: &Value, age_min: i64) -> BgTask {
    BgTask {
        id,
        kind: b["kind"].as_str().unwrap().into(),
        description: b["desc"].as_str().unwrap().into(),
        started: Some(NOW - age_min as f64 * 60.0),
    }
}

/// tests/test_states.py `states_swarm`: the design's dataset plus four orchestrators with
/// only their main session (two waiting, two stale), and n1 waiting on a CI watch.
pub fn states_swarm(data: &Value) -> Swarm {
    let mut sw = design_swarm(&fixture("design_data.json"));
    sw.docked = Some("w1:p5".into());
    let orchs = data["orchs"].as_array().unwrap();
    let by_tab = |tab: i64| orchs.iter().find(|o| o["tab"] == tab).unwrap();
    let known = sw.orchestrators.len() as i64;
    for o in &mut sw.orchestrators {
        let d = by_tab(o.tab_no.unwrap());
        o.last_activity = Some(NOW - d["last"].as_f64().unwrap() * 60.0);
        o.flavour = flavour(d["fl"].as_str().unwrap()).into();
    }
    for d in orchs.iter().filter(|d| d["tab"].as_i64().unwrap() > known) {
        let tab = d["tab"].as_i64().unwrap();
        let mut o: Orchestrator = serde_json::from_value(serde_json::json!({
            "pane_id": format!("w1:p{tab}"), "tab_id": format!("w1:t{tab}"),
            "tab_label": d["name"], "session_id": null, "herdr_status": "idle", "tab_no": tab,
            "flavour": flavour(d["fl"].as_str().unwrap()),
            "last_activity": NOW - d["last"].as_f64().unwrap() * 60.0,
        }))
        .unwrap();
        for (i, b) in d["bg"].as_array().unwrap().iter().enumerate() {
            let age = d["bgAge"]
                .as_i64()
                .unwrap_or_else(|| minutes(b["since"].as_str().unwrap()));
            o.bg.push(bg_task(format!("b{tab}{i}"), b, age));
        }
        sw.orchestrators.push(o);
    }
    let d1 = &data["n1"];
    let n1 = sw
        .orchestrators
        .iter_mut()
        .flat_map(|o| &mut o.threads)
        .flat_map(|t| &mut t.agents);
    let n1 = n1.into_iter().find(|a| a.id == "n1").unwrap();
    n1.status = Status::Background;
    n1.activity = d1["act"].as_str().unwrap().into();
    n1.last_message = d1["msg"].as_str().unwrap().into();
    n1.tool_calls = d1["tools"].as_i64().unwrap();
    n1.phases = d1["phs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_i64().unwrap())
        .collect();
    n1.started = Some(NOW - 74.0 * 60.0); // "1h14m"
    n1.bg = d1["bg"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| bg_task("n1b".into(), b, minutes(b["since"].as_str().unwrap())))
        .collect();
    sw
}
