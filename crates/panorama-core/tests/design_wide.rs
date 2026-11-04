//! The wide graph's depth columns at the views of the design's renders: turn 5's direction 5d
//! (`design_5d.json`, keys V4–V6) and turn 7's wider cards (`design_7.json`, keys W*, R* and
//! the sweep strip S). Python's tests/test_deep.py `test_5d_matches_the_design_cell_for_cell`
//! and tests/test_wide.py.
//!
//! The designs draw the thread column the uniform tree removed (plan `uniform-agent-tree.md`
//! U4, D5), so the pages are compared with Rust's own reviewed renders,
//! `tests/fixtures/rust/screens/design-{5d-wide,7}.json` (`PANORAMA_BLESS=1`). The sweep strip
//! has no thread in it and still matches the design.

mod support;

use panorama_core::model::{Status, Swarm};
use panorama_core::view::cards::Area;
use panorama_core::view::cards::boxes::rbox;
use panorama_core::view::cards::edge::edge;
use panorama_core::view::orch_state::OrchState;
use panorama_core::view::palette::LINE;
use panorama_core::view::{Screen, View, render_graph};
use support::owned::check_owned;
use support::tree_design::{NOW, tree_swarm};
use support::{NO_SESSIONS, fixture, golden, wire_swarm};

/// The view on the design's root with `agent` selected (else its first) and the keyboard in
/// column `focus`.
fn view_for(sw: &Swarm, agent: Option<&str>, focus: usize, frame: f64) -> View {
    let mut v = View {
        orch: Some("w1:p3".into()),
        focus,
        now: NOW,
        frame,
        ..View::default()
    };
    panorama_core::view::nav::default_children(sw, &mut v);
    if let Some(a) = agent {
        v.agent = Some(a.into());
    }
    v
}

fn frames(file: &serde_json::Value) -> Vec<String> {
    let keys: Vec<String> = file["frames"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert!(!keys.is_empty());
    keys
}

#[test]
fn the_tree_swarm_port_builds_what_python_built() {
    let ported = tree_swarm(&fixture("design_5d.json"));
    let captured = wire_swarm("inputs/swarm-deep.json", None);
    assert_eq!(ported, captured);
}

#[test]
fn design_5d_views() {
    let deep = fixture("design_5d.json");
    let sw: Swarm = wire_swarm("inputs/swarm-deep.json", None);
    // V4: the focus on the root (the depth columns to the right); V5: on b1 at depth 2;
    // V6: on c1 at depth 4
    for key in frames(&deep) {
        let frame: f64 = key.parse().unwrap();
        for (look, sel, focus) in [
            ("V4", None, 0),
            ("V5", Some("b1"), 2),
            ("V6", Some("c1"), 4),
        ] {
            let v = view_for(&sw, sel, focus, frame);
            let got = render_graph(&sw, &v, 160, 45, None, NO_SESSIONS);
            check_owned("design-5d-wide.json", &format!("{look} frame {key}"), &got);
        }
    }
}

/// The depth-2 agent tracing the manifest cache (test_wide.py `r1_id`).
fn r1(sw: &Swarm) -> String {
    sw.orchestrators[0]
        .agents()
        .find(|a| a.depth == 2 && a.activity.starts_with("tracing"))
        .unwrap()
        .id
        .clone()
}

#[test]
fn design_7_wide_cards() {
    let wide = fixture("design_7.json");
    let sw = tree_swarm(&wide);
    let r1 = r1(&sw);
    // W*: the focus on the root; R*: the focus on depth 2, the whole width
    let looks = [
        ("W160", 160, None),
        ("W240", 240, None),
        ("W320", 320, None),
        ("R160", 160, Some(&r1)),
        ("R320", 320, Some(&r1)),
    ];
    for key in frames(&wide) {
        let frame: f64 = key.parse().unwrap();
        for (look, w, sel) in looks {
            let focus = if sel.is_some() { 2 } else { 0 };
            let v = view_for(&sw, sel.map(String::as_str), focus, frame);
            let got = render_graph(&sw, &v, w, 45, None, NO_SESSIONS);
            check_owned("design-7.json", &format!("{look} frame {key}"), &got);
        }
    }
}

#[test]
fn design_7_sweep_strip() {
    // 28, 44, 56 and 60-column working cards side by side, as on the design page
    let wide = fixture("design_7.json");
    let working = OrchState {
        st: Status::Running,
        glyph: "⠋".into(),
        label: "working".into(),
        working: true,
        bg: 0,
    };
    for key in frames(&wide) {
        let mut s = Screen::new(206, 4);
        let mut x = 1;
        for w in [28, 44, 56, 60] {
            rbox(&mut s, x, 0, w, 3, LINE);
            edge(
                &mut s,
                Area { x, y: 0, w, h: 3 },
                &working,
                key.parse().unwrap(),
                0,
            );
            x += w + 3;
        }
        let want: String = golden(&wide["frames"][&key]["S"])[2]
            .iter()
            .map(|c| c.ch)
            .collect();
        let got: String = (0..206).map(|x| s.cell(x, 2).ch).collect();
        assert_eq!(got, want, "frame {key}");
    }
}
