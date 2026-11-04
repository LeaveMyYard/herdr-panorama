//! The cursor's rules the panel's own tests do not reach.

use super::Graph;
use crate::ui::selection::initial;
use panorama_core::model::Swarm;
use panorama_core::view::{Filter, NEW_ROW};
use serde_json::Value;

fn graph(name: &str, pane: &str) -> Graph {
    let path = format!(
        "{}/tests/fixtures/inputs/swarm-{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let swarm: Swarm = serde_json::from_value(v["swarm"].clone()).unwrap();
    let mut g = Graph {
        swarm,
        ..Graph::default()
    };
    initial(&g.swarm, &mut g.view, Some(pane));
    g
}

#[test]
fn the_selection_on_new_agent_is_nothing() {
    let mut g = graph("demo", "w1:p1");
    assert!(g.selection().0.is_some());
    g.view.orch = Some(NEW_ROW.into());
    let (o, a) = g.selection();
    assert!(o.is_none() && a.is_none());
}

#[test]
fn on_a_root_the_selection_is_the_root_alone() {
    // the remembered agent stays selected below it, but the keys act on the root
    let mut g = graph("demo", "w1:p1");
    assert_eq!(g.view.agent.as_deref(), Some("a1c3d90"));
    assert!(g.selection().1.is_none());
    g.step_right();
    assert_eq!(g.view.focus, 1);
    assert_eq!(g.selection().1.map(|a| a.id.as_str()), Some("a1c3d90"));
}

#[test]
fn down_from_a_cursor_parked_below_the_last_shown_row_lands_on_the_last_row() {
    // needs me shows w1:p1, w1:p3 and + New agent; parked off w1:p4 (below them all)
    let mut g = graph("demo", "w1:p1");
    g.view.filter = Filter::Needs;
    g.view.orch = None;
    g.parked = Some("w1:p4".into());
    g.move_cursor(1);
    assert_eq!(g.view.orch.as_deref(), Some(NEW_ROW));
}

#[test]
fn left_from_a_depth_1_agent_keeps_it_selected() {
    // the keyboard in a deeper column with a depth-1 agent selected (its column moved on):
    // ← steps back without re-selecting a parent it does not have
    let mut g = graph("deep", "w1:p3");
    g.point("w1:p3", Some("l1"));
    g.view.focus = 2;
    g.step_left();
    assert_eq!((g.view.focus, g.view.agent.as_deref()), (1, Some("l1")));
}

#[test]
fn right_and_left_walk_the_depths_from_the_root() {
    let mut g = graph("deep", "w1:p3");
    assert_eq!((g.view.focus, g.view.agent.as_deref()), (0, Some("l1")));
    g.step_right(); // the root's depth-1 agent
    assert_eq!((g.view.focus, g.view.agent.as_deref()), (1, Some("l1")));
    g.step_right(); // into what l1 spawned: its first shown child
    assert_eq!((g.view.focus, g.view.agent.as_deref()), (2, Some("r1")));
    g.step_left(); // back up to the agent that spawned it
    assert_eq!((g.view.focus, g.view.agent.as_deref()), (1, Some("l1")));
    g.step_left();
    assert_eq!(g.view.focus, 0);
    // from the root, a deeper agent selected: → steps into the depth-1 agent on its path
    g.point("w1:p3", Some("p1"));
    assert_eq!(g.view.focus, 5);
    g.focus(0);
    g.step_right();
    assert_eq!((g.view.focus, g.view.agent.as_deref()), (1, Some("l1")));
}
