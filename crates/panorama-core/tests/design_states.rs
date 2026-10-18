//! Turn 8, waiting and stale (docs/design/swarm-states.js rendered by render-8.mjs,
//! `design_8.json`): the docked panel cell for cell at whole frames 0, 4, 7, 12 and at the
//! fractional 1.5 and 3.4, where only the eased blink moves. The fractional frames are the
//! proof that rendering again at a fractional frame gives the design's colours (Python
//! restyles there instead). Python's tests/test_states.py, the panel half.

mod support;

use panorama_core::view::{Target, View, render_dock};
use support::design::design_swarm;
use support::states::{NOW, states_swarm};
use support::{NO_SESSIONS, assert_same, cells, fixture, golden, wire_swarm};

const H: i32 = 56;

#[test]
fn the_design_swarm_port_builds_what_python_built() {
    // design_dock.py's own output for design 9's data is captured in wire form
    let ported = design_swarm(&fixture("design_9.json")["data"]);
    let captured = wire_swarm("inputs/swarm-design9.json", Some("w1:p3"));
    assert_eq!(ported, captured);
}

#[test]
fn turn_8_panel_cell_for_cell_at_whole_and_fractional_frames() {
    let eight = fixture("design_8.json");
    let swarm = states_swarm(&eight["data"]);
    let frames = eight["frames"].as_object().unwrap();
    assert!(frames.contains_key("1.5") && frames.contains_key("3.4"));
    for (key, panes) in frames {
        let view = View {
            orch: Some("w1:p5".into()),
            hover: Some(Target::Orch("w1:p7".into())), // the design's hover
            frame: key.parse().unwrap(),
            now: NOW,
            ..View::default()
        };
        let got = render_dock(&swarm, &view, 46, H, None, NO_SESSIONS);
        assert_same(
            &format!("turn 8 frame {key}"),
            &golden(&panes["P"]),
            &cells(&got),
        );
    }
}
