//! `panorama tui` against every app-screen golden Python's `SwarmApp` captured
//! (`tests/fixtures/screens/*.json`, the `swarm` runs): each run's own steps (frames, keys,
//! hover, blur, selection) through the app headless, every snap cell for cell, the notices
//! included. A scenario run collects its materialised scenario at the run's clock, as the
//! capture's own source did.
//!
//! Not here: the snaps of the New agent form (P4 PR 9) and of a failed go-to (its notice is
//! the herdr side's, P4 8c).

mod collect_support;
mod screen_support;

use collect_support::{STALE, Scenario, WINDOW};
use panorama::proto::state::cleaned_swarm;
use panorama_core::model::Swarm;
use screen_support::player::Player;
use screen_support::{cells, fixture, golden};
use serde_json::Value;

/// The swarm a run showed: a captured wire swarm, or its scenario collected at the run's now
/// with the run's conditions (herdr down, no inbox, `PANORAMA_STALE_AFTER`).
fn swarm_of(run: &Value) -> Swarm {
    let input = run["input"].as_str().unwrap();
    if let Some(name) = input.strip_prefix("inputs/swarm-") {
        let wire = fixture(&format!("inputs/swarm-{name}"));
        return serde_json::from_value(wire["swarm"].clone()).unwrap();
    }
    let name = input
        .trim_start_matches("inputs/scenario-")
        .trim_end_matches(".json");
    let sc = Scenario::new(name);
    let env = &run["env"];
    let herdr_ok = env["FAKE_HERDR_FAIL"].as_str() != Some("api snapshot");
    let inbox_ok = env["HERDR_INBOX_DB"].is_null();
    let stale = match env["PANORAMA_STALE_AFTER"].as_str() {
        None => STALE,
        Some("30m") => 1800.0,
        Some(other) => panic!("a stale_after this test does not read: {other}"),
    };
    let questions = sc.questions();
    let swarm = sc.collector().collect(
        herdr_ok.then_some(sc.snapshot()),
        inbox_ok.then_some(questions.as_slice()),
        run["now"].as_f64().unwrap(),
        WINDOW,
        stale,
    );
    cleaned_swarm(swarm) // what the app's collect worker hands it
}

/// Every run of `file` played; its snaps compared (but `skip`'s, and the steps after them).
fn check(file: &str, skip: &[&str]) {
    let golden_file = fixture(&format!("screens/{file}"));
    let mut snapped = 0;
    for run in golden_file["runs"].as_array().unwrap() {
        let player = Player::new(run, &golden_file["unseen"], swarm_of(run));
        for (name, screen) in player.play(skip) {
            let (want, got) = (golden(&golden_file["screens"][&name]), cells(&screen));
            assert_eq!(want.len(), got.len(), "{file} {name}: rows");
            for (y, (w, g)) in want.iter().zip(&got).enumerate() {
                assert_eq!(
                    w,
                    g,
                    "{file} {name}: row {y}\n{}",
                    screen.text().lines().nth(y).unwrap_or("")
                );
            }
            snapped += 1;
        }
    }
    // every snap the file holds, but the skipped ones
    let held = golden_file["screens"].as_object().unwrap().len();
    assert_eq!(snapped, held - skip.len(), "{file}: snaps compared");
}

#[test]
fn demo_wide() {
    check("demo-wide.json", &["go-to failed toast"]);
}

#[test]
fn demo_narrow() {
    check("demo-narrow.json", &[]);
}

#[test]
fn demo_home() {
    check("demo-home.json", &["new agent form"]);
}

#[test]
fn dock() {
    check("dock.json", &[]);
}

#[test]
fn deep() {
    check("deep.json", &[]);
}

#[test]
fn remote() {
    check("remote-wide.json", &[]);
    check("remote-dock.json", &[]);
}

#[test]
fn scenarios() {
    for name in ["demo", "background", "continued"] {
        for layout in ["wide", "narrow", "dock"] {
            check(&format!("scenario-{name}-{layout}.json"), &[]);
        }
    }
}

#[test]
fn scenario_idle() {
    check("scenario-demo-idle.json", &[]);
}

#[test]
fn scenario_failures() {
    check("scenario-demo-failures.json", &[]);
}
