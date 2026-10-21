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
    let (o, t, a) = g.selection();
    assert!(o.is_none() && t.is_none() && a.is_none());
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
    g.point("w1:p3", Some("x:deep"), Some("l1"));
    g.view.focus = 3;
    g.step_left();
    assert_eq!((g.view.focus, g.view.agent.as_deref()), (2, Some("l1")));
}
