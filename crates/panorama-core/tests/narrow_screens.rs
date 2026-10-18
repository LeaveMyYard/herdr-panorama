//! The narrow layout against the app-screen goldens captured from Python's `SwarmApp` at 70
//! columns (`screens/demo-narrow.json`, `screens/scenario-*-narrow.json`), as first shown:
//! the focus pane selected, its default thread and agent (`nav::default_children`).

mod support;

use panorama_core::model::Swarm;
use panorama_core::view::nav::default_children;
use panorama_core::view::{View, render_narrow};
use serde_json::Value;
use support::{NO_SESSIONS, assert_same, cells, fixture, golden, wire_swarm};

/// The swarm a run showed: a captured wire swarm, or the model golden of a scenario at its
/// first instant (`panorama snapshot --json`, `+0m`).
fn swarm_of(run: &Value) -> Swarm {
    let input = run["input"].as_str().unwrap();
    if input.starts_with("inputs/swarm-") {
        return wire_swarm(input, None);
    }
    let name = input.trim_start_matches("inputs/scenario-");
    let mut snap = fixture(&format!("model/{name}"))["snapshots"]["+0m"].clone();
    snap.as_object_mut().unwrap().remove("counts");
    serde_json::from_value(snap).unwrap()
}

/// The screen `snap` of `file`'s run, with the cursor on `orch` (its default thread and agent)
/// in column `focus`: what the app's keys had left before that snap.
fn screen_with(file: &str, snap: &str, orch: Option<&str>, focus: usize) {
    let golden_file = fixture(&format!("screens/{file}"));
    let run = &golden_file["runs"][0];
    let swarm = swarm_of(run);
    let orch = orch.or(run["focus_pane"].as_str()).map(Into::into);
    let mut view = View {
        orch,
        now: run["now"].as_f64().unwrap(),
        ..View::default()
    };
    default_children(&swarm, &mut view);
    view.focus = focus;
    let (w, h) = (
        run["size"][0].as_i64().unwrap() as i32,
        run["size"][1].as_i64().unwrap() as i32,
    );
    let got = render_narrow(&swarm, &view, w, h, None, NO_SESSIONS);
    assert_same(
        &format!("{file} {snap}"),
        &golden(&golden_file["screens"][snap]),
        &cells(&got),
    );
}

fn first_screen(file: &str) {
    screen_with(file, "initial", None, 0);
}

// The demo run's key snaps, rendered from the View the keys left (the input PR proves the
// keys lead there): down moves to Search refactor (w1:p2), right into its threads, and after
// a down (it has one thread) right again into its agents.

#[test]
fn demo_key_1_down() {
    screen_with("demo-narrow.json", "key 1 down", Some("w1:p2"), 0);
}

#[test]
fn demo_key_2_right() {
    screen_with("demo-narrow.json", "key 2 right", Some("w1:p2"), 1);
}

#[test]
fn demo_key_4_right() {
    screen_with("demo-narrow.json", "key 4 right", Some("w1:p2"), 2);
}

#[test]
fn demo_at_70_columns() {
    first_screen("demo-narrow.json");
}

#[test]
fn scenario_demo_at_70_columns() {
    first_screen("scenario-demo-narrow.json");
}

#[test]
fn scenario_background_at_70_columns() {
    first_screen("scenario-background-narrow.json");
}

#[test]
fn scenario_continued_at_70_columns() {
    first_screen("scenario-continued-narrow.json");
}
