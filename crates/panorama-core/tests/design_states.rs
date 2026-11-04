//! Turn 8, waiting and stale (docs/design/swarm-states.js rendered by render-8.mjs,
//! `design_8.json`): the docked panel cell for cell at whole frames 0, 4, 7, 12 and at the
//! fractional 1.5 and 3.4, where only the eased blink moves. The fractional frames are the
//! proof that rendering again at a fractional frame gives the design's colours (Python
//! restyles there instead). Python's tests/test_states.py: the panel, and the 2a graph's
//! root cards, a root's background work and the waiting agent card. The panel's screens are
//! Rust's own since U5 (D5): `rust/screens/design-8.json`, written by `PANORAMA_BLESS=1`.
//!
//! The design's 2a graph draws the thread column the uniform tree removed (plan
//! `uniform-agent-tree.md` U4, D5): its pages are compared with Rust's own reviewed renders,
//! `tests/fixtures/rust/screens/design-8-wide.json` (`PANORAMA_BLESS=1`).

mod support;

use panorama_core::model::{BgTask, Swarm};
use panorama_core::view::graph::layout::tier;
use panorama_core::view::{Filter, Target, View, render_dock, render_graph};
use support::design::design_swarm;
use support::owned::check_owned;
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
    for key in frames.keys() {
        let view = View {
            orch: Some("w1:p5".into()),
            hover: Some(Target::Orch("w1:p7".into())), // the design's hover
            frame: key.parse().unwrap(),
            now: NOW,
            ..View::default()
        };
        let got = render_dock(&swarm, &view, 46, H, None, NO_SESSIONS);
        check_owned("design-8.json", &format!("P@{key}"), &got);
    }
}
/// The 2a graph at `frame`, 160 × 56: `sel`'s root and agent, the keyboard in `focus`.
fn graph(
    swarm: &Swarm,
    frame: f64,
    sel: (&str, Option<&str>),
    focus: usize,
    filter: Filter,
) -> panorama_core::view::Screen {
    let mut view = View {
        orch: Some(sel.0.into()),
        focus,
        frame,
        now: NOW,
        filter,
        ..View::default()
    };
    panorama_core::view::nav::default_children(swarm, &mut view);
    if let Some(a) = sel.1 {
        view.agent = Some(a.into());
    }
    render_graph(swarm, &view, 160, H, None, NO_SESSIONS)
}

#[test]
fn turn_8_2a_roots_and_a_roots_background_work() {
    // A: the keyboard on the root with only its own background tasks: its card lists them
    let eight = fixture("design_8.json");
    let swarm = states_swarm(&eight["data"]);
    for key in eight["frames"].as_object().unwrap().keys() {
        let got = graph(
            &swarm,
            key.parse().unwrap(),
            ("w1:p5", None),
            0,
            Filter::All,
        );
        check_owned("design-8-wide.json", &format!("A frame {key}"), &got);
    }
}

#[test]
fn turn_8_2a_waiting_agent_card() {
    let eight = fixture("design_8.json");
    let swarm = states_swarm(&eight["data"]);
    for key in eight["frames"].as_object().unwrap().keys() {
        let sel = ("w1:p4", Some("n1"));
        let got = graph(&swarm, key.parse().unwrap(), sel, 1, Filter::All);
        check_owned("design-8-wide.json", &format!("B frame {key}"), &got);
    }
}

#[test]
fn turn_8_running_filter_keeps_waiting_and_drops_stale() {
    let eight = fixture("design_8.json");
    let swarm = states_swarm(&eight["data"]);
    for key in eight["frames"].as_object().unwrap().keys() {
        let sel = ("w1:p4", Some("n1"));
        let got = graph(&swarm, key.parse().unwrap(), sel, 1, Filter::Running);
        // 6 root cards: the stale ones are gone
        let mut roots: Vec<&Target> = got
            .targets
            .iter()
            .map(|h| &h.target)
            .filter(|t| matches!(t, Target::Orch(_)))
            .collect();
        roots.dedup();
        assert_eq!(roots.len(), 6, "R frame {key}:\n{}", got.text());
        check_owned("design-8-wide.json", &format!("R frame {key}"), &got);
    }
}

#[test]
fn a_roots_background_work_keeps_inside_its_card() {
    // test_states.py::test_the_background_card_takes_the_wide_thread_column, on the uniform
    // tree: the root's background tasks are rows of its own card, cut at its border
    let mut swarm = states_swarm(&fixture("design_8.json")["data"]);
    swarm.orchestrators[3].bg = vec![BgTask {
        id: "x".into(),
        kind: "Bash".into(),
        description: "a long description ".repeat(4),
        started: Some(NOW - 60.0),
    }];
    let pane = swarm.orchestrators[3].pane_id.clone();
    let view = View {
        orch: Some(pane),
        focus: 0,
        now: NOW,
        ..View::default()
    };
    for w in [160, 240, 320] {
        let s = render_graph(&swarm, &view, w, H, None, NO_SESSIONS);
        let text = s.text();
        let y = text
            .lines()
            .position(|r| r.contains("a long description"))
            .unwrap_or_else(|| panic!("{w}:\n{text}"));
        let right = 2 + tier(w).0 - 1;
        assert_eq!(
            s.cell(right, y as i32).ch,
            '│',
            "{w}: {}",
            text.lines().nth(y).unwrap()
        );
        let row: String = (2..=right).map(|x| s.cell(x, y as i32).ch).collect();
        assert!(row.contains('…'), "{w}: cut at its border: {row}");
    }
}
