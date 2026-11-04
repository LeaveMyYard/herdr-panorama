//! Subagents to any depth in the docked panel, on design 5d's deep dataset
//! (`inputs/swarm-deep.json`, the frames of `design_5d.json`). Design 5d drew the thread and
//! the spawner tree below it; since the uniform agent tree (plan `uniform-agent-tree.md` U5,
//! D5) the docked list is the root's agents in two layers, and its screens are Rust's own,
//! `rust/screens/design-5d-dock.json`, written by `PANORAMA_BLESS=1`.

mod support;

use panorama_core::view::{View, render_dock};
use support::owned::check_owned;
use support::{NO_SESSIONS, fixture, wire_swarm};

/// tests/design_deep.py `NOW`.
const NOW: f64 = 1_800_000_000.0;

#[test]
fn the_docked_tree_rows_are_two_layers_of_the_roots_agents() {
    let deep = fixture("design_5d.json");
    let mut sw = wire_swarm("inputs/swarm-deep.json", None);
    sw.docked = Some("w1:p3".into());
    let frames = deep["frames"].as_object().unwrap();
    assert!(!frames.is_empty());
    for key in frames.keys() {
        let v = View {
            orch: Some("w1:p3".into()),
            now: NOW,
            frame: key.parse().unwrap(),
            ..View::default()
        };
        let s = render_dock(&sw, &v, 46, 45, None, NO_SESSIONS);
        check_owned("design-5d-dock.json", &format!("P4@{key}"), &s);
    }
}
