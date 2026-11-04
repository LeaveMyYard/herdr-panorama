//! The model goldens (plan §5.2 item 3): `panorama snapshot --json` over each captured
//! scenario at the capture's times and conditions, byte for byte.
//!
//! Python parity is retired for them (plan `uniform-agent-tree.md` U3, R3): the times,
//! conditions and inputs are still those `tools/capture_goldens.py` captured from the Python
//! (`tests/fixtures/model/*.json`, kept as Python's own record), but the expected models are
//! Rust's, `tests/fixtures/rust/model/*.json`, rewritten by
//! `PANORAMA_BLESS=1 cargo test --test model_goldens` and reviewed as a diff. They differ from
//! Python's in the agent tree (one thread per root, no session card, no phases, D4's window),
//! the titles and `harness` (`docs/plans/rust-rewrite.md`, "Divergence").

#[path = "screen_support/bless.rs"]
mod bless;
mod collect_support;

use bless::blessing;
use collect_support::{MODELS, STALE, Scenario, WINDOW, read_json};

const RUST_MODELS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/rust/model");
use panorama::snapshot::pyjson::{self, J};
use panorama::snapshot::{Format, json, render};
use serde_json::Value;
use std::path::Path;

/// `capture_goldens.py`'s `MODEL_TIMES`: minutes after the scenario's own now.
const MODEL_TIMES: [(&str, &[i64]); 3] = [
    ("demo", &[0, 30, 180, 420, 780]),
    ("continued", &[0, 120]),
    ("background", &[0, 60]),
];
/// Its `CONDITIONS`, run on the demo at its own now, in this order.
const CONDITIONS: [&str; 2] = ["herdr down", "no inbox"];

/// One `snapshot --json` at `now` with a condition's env: herdr failing `api snapshot`, or an
/// inbox db that does not exist. A fresh collector each time, as each capture ran the CLI.
fn snapshot(sc: &Scenario, now: f64, env: Option<&Value>) -> J {
    let mut herdr_ok = true;
    let mut inbox_ok = true;
    for (key, value) in env.and_then(Value::as_object).into_iter().flatten() {
        let value = value.as_str().unwrap();
        match key.as_str() {
            "FAKE_HERDR_FAIL" => herdr_ok = !value.split(',').any(|c| c == "api snapshot"),
            "HERDR_INBOX_DB" => inbox_ok = value == "<root>/inbox.db",
            other => panic!("a condition this test does not model: {other}"),
        }
    }
    let questions = sc.questions();
    let swarm = sc.collector().collect(
        herdr_ok.then_some(sc.snapshot()),
        inbox_ok.then_some(questions.as_slice()),
        now,
        WINDOW,
        STALE,
    );
    // what `--json` prints (`render` adds only the newline); the capture replaced the
    // materialised root with "<root>"
    assert_eq!(
        render(&swarm, Format::Json, 0.0),
        pyjson::dumps(&json::swarm(&swarm), 2) + "\n"
    );
    // `snapshot --json` names every orchestrator and agent
    for o in &swarm.roots {
        assert!(
            !o.title.is_empty() && !o.title_source.is_empty(),
            "{}",
            o.pane_id
        );
        for a in o.agents.iter() {
            assert!(
                !a.title.is_empty() && !a.title_source.is_empty(),
                "{}",
                a.id
            );
        }
    }
    json::swarm(&swarm).replace_in_strings(&sc.root().to_string_lossy(), "<root>")
}

fn check(name: &str, minutes: &[i64]) {
    // the capture's inputs, times and conditions
    let golden = read_json(&Path::new(MODELS).join(format!("{name}.json")));
    let sc = Scenario::new(name);
    let now = sc.now();
    assert_eq!(golden["now"].as_f64(), Some(now));
    let mut runs: Vec<(String, f64, Option<&Value>)> = minutes
        .iter()
        .map(|m| (format!("+{m}m"), now + *m as f64 * 60.0, None))
        .collect();
    if name == "demo" {
        for cond in CONDITIONS {
            let key = format!("+0m {cond}");
            runs.push((key.clone(), now, Some(&golden["conditions"][&key])));
        }
    }
    let snaps: Vec<(String, J)> = runs
        .iter()
        .map(|(k, at, env)| (k.clone(), snapshot(&sc, *at, *env)))
        .collect();
    let conditions = J::Obj(
        runs.iter()
            .filter_map(|(k, _, env)| env.map(|e| (k.clone(), J::from(e))))
            .collect(),
    );
    let doc = J::obj([
        (
            "source",
            J::str(
                "Rust's `panorama snapshot --json` at the times and conditions of \
                 tests/fixtures/model (Python's capture), written by \
                 PANORAMA_BLESS=1 cargo test --test model_goldens",
            ),
        ),
        ("input", J::from(&golden["input"])),
        ("now", J::Float(now)),
        ("conditions", conditions),
        ("snapshots", J::Obj(snaps)),
    ]);
    let written = pyjson::dumps(&doc, 1) + "\n";
    let rust_path = Path::new(RUST_MODELS).join(format!("{name}.json"));
    if blessing() {
        std::fs::create_dir_all(RUST_MODELS).unwrap();
        std::fs::write(&rust_path, &written).unwrap();
        return;
    }
    let golden_text = std::fs::read_to_string(&rust_path).unwrap();
    if written != golden_text {
        let line = written
            .lines()
            .zip(golden_text.lines())
            .position(|(a, b)| a != b)
            .unwrap_or(0);
        panic!(
            "{name}: the model differs from the golden at line {}:\n got: {:?}\nwant: {:?}",
            line + 1,
            written.lines().nth(line),
            golden_text.lines().nth(line)
        );
    }
}

#[test]
fn the_demo_model_matches_its_golden() {
    check("demo", MODEL_TIMES[0].1);
}

#[test]
fn the_continued_model_matches_its_golden() {
    check("continued", MODEL_TIMES[1].1);
}

#[test]
fn the_background_model_matches_its_golden() {
    check("background", MODEL_TIMES[2].1);
}

#[test]
fn every_golden_is_checked() {
    for dir in [MODELS, RUST_MODELS] {
        check_names(dir);
    }
}

fn check_names(dir: &str) {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    let mut known: Vec<String> = MODEL_TIMES
        .iter()
        .map(|(n, _)| format!("{n}.json"))
        .collect();
    known.sort();
    assert_eq!(names, known);
}
