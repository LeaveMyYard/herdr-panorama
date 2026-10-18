//! The wide graph against the app-screen goldens captured from Python's `SwarmApp` at 160
//! columns (`screens/demo-wide.json`, `deep.json`, `remote-wide.json`,
//! `scenario-*-wide.json`, and `scenario-demo-failures.json`'s "no inbox"), rendered from
//! the View the app's keys left before each snap (its default thread and agent:
//! `nav::default_children`) with the unseen marks it drew (the file's `unseen`). The toasts
//! are the app's, not the view's (P4 8a).

mod support;

use panorama_core::model::Swarm;
use panorama_core::view::nav::default_children;
use panorama_core::view::{Filter, View, render_graph};
use serde_json::Value;
use support::{NO_SESSIONS, assert_same, cells, fixture, golden, wire_swarm};

/// The swarm a run showed: a captured wire swarm, or the model golden of a scenario at its
/// first instant (`panorama snapshot --json`; `cond`: one of its conditions).
fn swarm_of(run: &Value, cond: &str) -> Swarm {
    let input = run["input"].as_str().unwrap();
    if input.starts_with("inputs/swarm-") {
        return wire_swarm(input, None);
    }
    let name = input.trim_start_matches("inputs/scenario-");
    let mut snap = fixture(&format!("model/{name}"))["snapshots"][format!("+0m{cond}")].clone();
    snap.as_object_mut().unwrap().remove("counts");
    serde_json::from_value(snap).unwrap()
}

/// What the keys left before a snap.
#[derive(Clone, Copy, Default)]
struct Keys<'a> {
    orch: Option<&'a str>,
    focus: usize,
    frame: f64,
    filter: Option<Filter>,
    /// A `select` step: the thread and agent it set.
    select: Option<(&'a str, Option<&'a str>)>,
}

/// The screen `snap` of `file`'s run `run` after `keys`.
fn screen_with(file: &str, run: usize, snap: &str, cond: &str, keys: Keys) {
    let golden_file = fixture(&format!("screens/{file}"));
    let run = &golden_file["runs"][run];
    let swarm = swarm_of(run, cond);
    let mut view = View {
        orch: keys.orch.or(run["focus_pane"].as_str()).map(Into::into),
        now: run["now"].as_f64().unwrap(),
        filter: keys.filter.unwrap_or_default(),
        frame: keys.frame,
        ..View::default()
    };
    view.unseen = serde_json::from_value(golden_file["unseen"][snap].clone()).unwrap();
    default_children(&swarm, &mut view);
    if let Some((thread, agent)) = keys.select {
        view.thread = Some(thread.into());
        view.agent = agent.map(Into::into);
    }
    view.focus = keys.focus;
    let size = &run["size"];
    let (w, h) = (
        size[0].as_i64().unwrap() as i32,
        size[1].as_i64().unwrap() as i32,
    );
    let got = render_graph(&swarm, &view, w, h, None, NO_SESSIONS);
    let want = golden(&golden_file["screens"][snap]);
    assert_same(&format!("{file} {snap}"), &want, &cells(&got));
}

#[test]
fn demo_initial() {
    screen_with("demo-wide.json", 0, "initial", "", Keys::default());
}

#[test]
fn demo_initial_at_frame_4() {
    let keys = Keys {
        frame: 4.0,
        ..Keys::default()
    };
    screen_with("demo-wide.json", 0, "initial@frame4", "", keys);
}

// down moves to Search refactor (w1:p2), right into its threads, and after a down (it has one
// thread) right again into its agents

#[test]
fn demo_key_1_down() {
    let keys = Keys {
        orch: Some("w1:p2"),
        ..Keys::default()
    };
    screen_with("demo-wide.json", 0, "key 1 down", "", keys);
}

#[test]
fn demo_key_2_right() {
    let keys = Keys {
        orch: Some("w1:p2"),
        focus: 1,
        ..Keys::default()
    };
    screen_with("demo-wide.json", 0, "key 2 right", "", keys);
}

#[test]
fn demo_key_4_right() {
    let keys = Keys {
        orch: Some("w1:p2"),
        focus: 2,
        ..Keys::default()
    };
    screen_with("demo-wide.json", 0, "key 4 right", "", keys);
}

#[test]
fn demo_filter_running() {
    let keys = Keys {
        orch: Some("w1:p2"),
        focus: 2,
        filter: Some(Filter::Running),
        ..Keys::default()
    };
    screen_with("demo-wide.json", 0, "filter running", "", keys);
}

#[test]
fn demo_filter_needs() {
    // Search refactor needs nothing: the selection falls back to the first orchestrator shown
    let keys = Keys {
        orch: Some("w1:p1"),
        focus: 2,
        filter: Some(Filter::Needs),
        ..Keys::default()
    };
    screen_with("demo-wide.json", 0, "filter needs", "", keys);
}

#[test]
fn deep_thread() {
    let keys = Keys {
        focus: 1,
        select: Some(("x:deep", None)),
        ..Keys::default()
    };
    screen_with("deep.json", 0, "thread", "", keys);
}

#[test]
fn deep_c1() {
    let keys = Keys {
        focus: 5,
        select: Some(("x:deep", Some("c1"))),
        ..Keys::default()
    };
    screen_with("deep.json", 0, "c1", "", keys);
}

#[test]
fn remote_initial() {
    screen_with("remote-wide.json", 0, "initial", "", Keys::default());
}

#[test]
fn scenarios_initial_and_key_1_right() {
    for name in ["demo", "background", "continued"] {
        let file = format!("scenario-{name}-wide.json");
        screen_with(&file, 0, "initial", "", Keys::default());
        let keys = Keys {
            focus: 1,
            ..Keys::default()
        };
        screen_with(&file, 0, "key 1 right", "", keys);
    }
}

#[test]
fn scenario_demo_without_the_inbox() {
    screen_with(
        "scenario-demo-failures.json",
        1,
        "no inbox",
        " no inbox",
        Keys::default(),
    );
}
