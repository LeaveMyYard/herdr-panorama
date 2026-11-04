//! The wide graph against the app-screen goldens captured from Python's `SwarmApp` at 160
//! columns (`screens/demo-wide.json`, `deep.json`, `remote-wide.json`), rendered from
//! the View the app's keys left before each snap (its default agent:
//! `nav::default_children`) with the unseen marks it drew (the file's `unseen`). The toasts
//! are the app's, not the view's (P4 8a).
//!
//! Since the uniform tree (plan `uniform-agent-tree.md` U4, D5) the graph's columns are
//! depths, so the screens are Rust's own, `tests/fixtures/rust/screens/<file>`, which the app
//! writes playing the same runs (`tests/app_screens.rs`, `PANORAMA_BLESS=1`): the view a
//! snap's keys leave is drawn the same here as there. The scenario captures are the app's
//! alone (`app_screens`): their swarms are collected there, at the runs' own times.

mod support;

use panorama_core::view::nav::default_children;
use panorama_core::view::{Filter, View, render_graph};
use support::{NO_SESSIONS, assert_same, cells, fixture, golden, wire_swarm};

/// What the keys left before a snap.
#[derive(Clone, Copy, Default)]
struct Keys<'a> {
    orch: Option<&'a str>,
    focus: usize,
    frame: f64,
    filter: Option<Filter>,
    /// The agent the keys left selected, when not the default one.
    select: Option<&'a str>,
}

/// The screen `snap` of `file`'s run `run` after `keys`.
fn screen_with(file: &str, run: usize, snap: &str, keys: Keys) {
    let golden_file = fixture(&format!("screens/{file}"));
    let run = &golden_file["runs"][run];
    let swarm = wire_swarm(run["input"].as_str().unwrap(), None);
    let mut view = View {
        orch: keys.orch.or(run["focus_pane"].as_str()).map(Into::into),
        now: run["now"].as_f64().unwrap(),
        filter: keys.filter.unwrap_or_default(),
        frame: keys.frame,
        ..View::default()
    };
    view.unseen = serde_json::from_value(golden_file["unseen"][snap].clone()).unwrap();
    default_children(&swarm, &mut view);
    if let Some(agent) = keys.select {
        view.agent = Some(agent.into());
    }
    view.focus = keys.focus;
    let size = &run["size"];
    let (w, h) = (
        size[0].as_i64().unwrap() as i32,
        size[1].as_i64().unwrap() as i32,
    );
    let got = render_graph(&swarm, &view, w, h, None, NO_SESSIONS);
    let want = golden(&fixture(&format!("rust/screens/{file}"))["screens"][snap]);
    assert_same(&format!("{file} {snap}"), &want, &cells(&got));
}

#[test]
fn demo_initial() {
    screen_with("demo-wide.json", 0, "initial", Keys::default());
}

#[test]
fn demo_initial_at_frame_4() {
    let keys = Keys {
        frame: 4.0,
        ..Keys::default()
    };
    screen_with("demo-wide.json", 0, "initial@frame4", keys);
}

// down moves to Search refactor (w1:p2), right into its depth-1 agents, and after a down to
// the second one, right again does nothing (it spawned none)

#[test]
fn demo_key_1_down() {
    let keys = Keys {
        orch: Some("w1:p2"),
        ..Keys::default()
    };
    screen_with("demo-wide.json", 0, "key 1 down", keys);
}

#[test]
fn demo_key_2_right() {
    let keys = Keys {
        orch: Some("w1:p2"),
        focus: 1,
        ..Keys::default()
    };
    screen_with("demo-wide.json", 0, "key 2 right", keys);
}

#[test]
fn demo_key_4_right() {
    let keys = Keys {
        orch: Some("w1:p2"),
        focus: 1,
        select: Some("j4d5e6f"),
        ..Keys::default()
    };
    screen_with("demo-wide.json", 0, "key 4 right", keys);
}

#[test]
fn demo_filter_running() {
    let keys = Keys {
        orch: Some("w1:p2"),
        focus: 1,
        select: Some("j4d5e6f"),
        filter: Some(Filter::Running),
        ..Keys::default()
    };
    screen_with("demo-wide.json", 0, "filter running", keys);
}

#[test]
fn demo_filter_needs() {
    // Search refactor needs nothing: the selection falls back to the first orchestrator shown
    let keys = Keys {
        orch: Some("w1:p1"),
        focus: 1,
        filter: Some(Filter::Needs),
        ..Keys::default()
    };
    screen_with("demo-wide.json", 0, "filter needs", keys);
}

#[test]
fn deep_thread() {
    // Python's thread selected: its root here
    let keys = Keys::default();
    screen_with("deep.json", 0, "thread", keys);
}

#[test]
fn deep_c1() {
    let keys = Keys {
        focus: 4,
        select: Some("c1"),
        ..Keys::default()
    };
    screen_with("deep.json", 0, "c1", keys);
}

#[test]
fn remote_initial() {
    screen_with("remote-wide.json", 0, "initial", Keys::default());
}

/// The U4 review renders (plan `uniform-agent-tree.md` U4: the operator's check at 160 and
/// 240 columns) on a synthetic uniform swarm, `rust/inputs/swarm-uniform.json`: one tree per root,
/// titled agents, a root's own questions, prompt and background work, the window's hidden
/// finished agents, a tree five deep. Rust's own reviewed renders, `rust/screens/uniform-wide.json`.
#[test]
fn the_uniform_tree_at_160_and_240_columns() {
    let rel = "rust/inputs/swarm-uniform.json";
    let swarm = wire_swarm(rel, None);
    let now = fixture(rel)["now"].as_f64().unwrap();
    for (name, w, pane, agent, focus) in [
        ("160 Billing API, the root", 160, "w1:p1", None, 0),
        ("160 Billing API, depth 2", 160, "w1:p1", Some("a1c1"), 2),
        ("160 Release notes, all finished", 160, "w1:p4", None, 0),
        (
            "240 Search refactor, depth 1",
            240,
            "w1:p2",
            Some("s110"),
            1,
        ),
        (
            "240 Search refactor, depth 4",
            240,
            "w1:p2",
            Some("s113"),
            4,
        ),
        ("240 Docs site, the root", 240, "w1:p3", None, 0),
    ] {
        let mut view = View {
            orch: Some(pane.into()),
            now,
            ..View::default()
        };
        default_children(&swarm, &mut view);
        if let Some(a) = agent {
            view.agent = Some(a.into());
        }
        view.focus = focus;
        let got = render_graph(&swarm, &view, w, 45, None, NO_SESSIONS);
        support::owned::check_owned("uniform-wide.json", name, &got);
    }
}
