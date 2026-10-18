//! The wide graph's depth columns against the design's renders, cell for cell: turn 5's
//! direction 5d (`design_5d.json`, keys V4–V6) and turn 7's wider cards
//! (`design_7.json`, keys W*, R* and the sweep strip S). Python's tests/test_deep.py
//! `test_5d_matches_the_design_cell_for_cell` and tests/test_wide.py.

mod support;

use panorama_core::model::{Status, Swarm};
use panorama_core::view::cards::Area;
use panorama_core::view::cards::boxes::rbox;
use panorama_core::view::cards::edge::edge;
use panorama_core::view::orch_state::OrchState;
use panorama_core::view::palette::LINE;
use panorama_core::view::{Screen, View, render_graph};
use support::tree_design::{NOW, TID, tree_swarm};
use support::{NO_SESSIONS, Shown, cells, fixture, golden, wire_swarm};

/// The view on the design's thread with `agent` selected in column `focus`.
fn view_for(agent: Option<&str>, focus: usize, frame: f64) -> View {
    View {
        orch: Some("w1:p3".into()),
        thread: Some(TID.into()),
        agent: agent.map(Into::into),
        focus,
        now: NOW,
        frame,
        ..View::default()
    }
}

/// Rows 2..41 from column `x0` on: the line above the columns and the columns.
fn assert_rows(name: &str, want: &[Vec<Shown>], got: &[Vec<Shown>], x0: usize) {
    let mut diffs = vec![];
    for y in 2..42 {
        for x in x0..want[y].len() {
            if want[y][x] != got[y][x] {
                let row: String = got[y].iter().map(|c| c.ch).collect();
                diffs.push(format!(
                    "({x},{y}) want {:?} got {:?}\n  got row {row:?}",
                    want[y][x], got[y][x]
                ));
            }
        }
    }
    assert!(
        diffs.is_empty(),
        "{name}: {} cells differ, first:\n{}",
        diffs.len(),
        diffs[..diffs.len().min(3)].join("\n")
    );
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
fn design_5d_cell_for_cell() {
    let deep = fixture("design_5d.json");
    let sw: Swarm = wire_swarm("inputs/swarm-deep.json", None);
    // V4: the focus on the thread (2a, the depth columns to the right; compared from the
    // thread→agent links); V5: on b1 at depth 2; V6: on c1 at depth 4
    for key in frames(&deep) {
        let frame: f64 = key.parse().unwrap();
        for (look, sel, focus, x0) in [
            ("V4", None, 1, 89),
            ("V5", Some("b1"), 3, 0),
            ("V6", Some("c1"), 5, 0),
        ] {
            let got = render_graph(
                &sw,
                &view_for(sel, focus, frame),
                160,
                45,
                None,
                NO_SESSIONS,
            );
            assert_rows(
                &format!("5d {look} frame {key}"),
                &golden(&deep["frames"][&key][look]),
                &cells(&got),
                x0,
            );
        }
    }
}

/// The depth-2 agent tracing the manifest cache (test_wide.py `r1_id`).
fn r1(sw: &Swarm) -> String {
    let t = &sw.orchestrators[0].threads[0];
    t.agents
        .iter()
        .find(|a| a.depth == 2 && a.activity.starts_with("tracing"))
        .unwrap()
        .id
        .clone()
}

#[test]
fn design_7_wide_cards_cell_for_cell() {
    let wide = fixture("design_7.json");
    let sw = tree_swarm(&wide);
    let r1 = r1(&sw);
    // W*: the focus on the thread (from the thread→agent links rightwards: the orchestrator
    // and thread columns are the design's mock); R*: the focus on depth 2, the whole width
    let looks = [
        ("W160", 160, None, 89),
        ("W240", 240, None, 99),
        ("W320", 320, None, 111),
        ("R160", 160, Some(&r1), 0),
        ("R320", 320, Some(&r1), 0),
    ];
    for key in frames(&wide) {
        let frame: f64 = key.parse().unwrap();
        for (look, w, sel, x0) in looks {
            let focus = if sel.is_some() { 3 } else { 1 };
            let got = render_graph(
                &sw,
                &view_for(sel.map(String::as_str), focus, frame),
                w,
                45,
                None,
                NO_SESSIONS,
            );
            assert_rows(
                &format!("7 {look} frame {key}"),
                &golden(&wide["frames"][&key][look]),
                &cells(&got),
                x0,
            );
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
