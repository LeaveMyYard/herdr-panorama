//! No agent is lost to the uniform tree (plan `uniform-agent-tree.md` U3, R7): over every
//! captured scenario, at each time, window and inbox state the collector was captured at before
//! U3 (`tests/fixtures/collect/agent-ids-pre-u3.json`), each root holds exactly the agents it
//! held then, but for the two changes the plan makes on purpose:
//! - the session's own `main:` card is gone (its state is on the root);
//! - a failed agent ages out of the done window like a done one (D4), unless an agent it
//!   spawned is still shown.
//!
//! No captured root has a failed agent older than its window that was still shown before U3
//! (the old thread window had hidden it too), so the second exception is the rule, not
//! exercised here: `collect_demo` ages a failed agent out, `collect_deep` keeps an ancestor.

mod collect_support;

use collect_support::{STALE, Scenario, read_json};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

const BASELINE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/collect/agent-ids-pre-u3.json"
);

/// The ids a root held before U3, less the two deliberate changes.
fn expected(agents: &[Value], now: f64, window: f64) -> BTreeSet<String> {
    let id = |a: &Value| a["id"].as_str().unwrap().to_owned();
    let parent: HashMap<String, String> = agents
        .iter()
        .filter_map(|a| Some((id(a), a["parent"].as_str()?.to_owned())))
        .collect();
    let aged = |a: &Value| a["status"] == "fail" && a["end"].as_f64().unwrap_or(0.0) < now - window;
    let mut kept: BTreeSet<String> = agents
        .iter()
        .filter(|a| !id(a).starts_with("main:") && !aged(a))
        .map(id)
        .collect();
    // every agent between a shown one and its top stays
    let known: BTreeSet<String> = agents.iter().map(id).collect();
    for a in kept.clone() {
        let mut cur = parent.get(&a);
        while let Some(p) = cur.filter(|p| known.contains(*p) && !p.starts_with("main:")) {
            if !kept.insert(p.clone()) {
                break;
            }
            cur = parent.get(p);
        }
    }
    kept
}

#[test]
fn every_scenario_keeps_its_agents_but_the_main_card_and_aged_failures() {
    let baseline = read_json(Path::new(BASELINE));
    let mut checked = 0;
    for (name, runs) in baseline["runs"].as_object().unwrap() {
        let sc = match name.strip_prefix("scenario-") {
            Some(s) => Scenario::new(s),
            None => Scenario::load(
                &Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{name}.json")),
            ),
        };
        for (key, run) in runs.as_object().unwrap() {
            let (now, window) = (
                run["now"].as_f64().unwrap(),
                run["window"].as_f64().unwrap(),
            );
            let qs = sc.questions();
            let inbox = run["inbox"].as_bool().unwrap().then_some(qs.as_slice());
            let sw = sc
                .collector()
                .collect(Some(sc.snapshot()), inbox, now, window, STALE);
            let old = run["roots"].as_array().unwrap();
            let panes: Vec<&str> = sw.roots.iter().map(|o| o.pane_id.as_str()).collect();
            let want: Vec<&str> = old.iter().map(|r| r["pane"].as_str().unwrap()).collect();
            assert_eq!(panes, want, "{name} {key}: the roots");
            for (o, r) in sw.roots.iter().zip(old) {
                let got: Vec<String> = o.agents.iter().map(|a| a.id.clone()).collect();
                let unique: BTreeSet<String> = got.iter().cloned().collect();
                assert_eq!(unique.len(), got.len(), "{name} {key}: a repeated agent");
                let before = r["agents"].as_array().unwrap();
                let want = expected(before, now, window);
                assert_eq!(unique, want, "{name} {key} {}", o.pane_id);
                checked += 1;
            }
        }
    }
    assert!(checked >= 60, "{checked} roots checked");
}
