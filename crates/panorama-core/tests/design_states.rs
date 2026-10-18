//! Turn 8, waiting and stale (docs/design/swarm-states.js rendered by render-8.mjs,
//! `design_8.json`): the docked panel cell for cell at whole frames 0, 4, 7, 12 and at the
//! fractional 1.5 and 3.4, where only the eased blink moves. The fractional frames are the
//! proof that rendering again at a fractional frame gives the design's colours (Python
//! restyles there instead). Python's tests/test_states.py: the panel, and the 2a graph's
//! orchestrator cards, background card and waiting agent card.

mod support;

use panorama_core::model::{BgTask, Swarm};
use panorama_core::view::state::BG_CARD;
use panorama_core::view::{Filter, Target, View, render_dock, render_graph};
use support::design::design_swarm;
use support::states::{NOW, states_swarm};
use support::{NO_SESSIONS, Shown, assert_same, cells, fixture, golden, wire_swarm};

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
/// The 2a graph at `frame`, 160 × 56, the keyboard in the agents' column.
fn graph(
    swarm: &Swarm,
    frame: f64,
    sel: (&str, Option<&str>, Option<&str>),
    filter: Filter,
) -> Vec<Vec<Shown>> {
    let view = View {
        orch: Some(sel.0.into()),
        thread: sel.1.map(Into::into),
        agent: sel.2.map(Into::into),
        focus: 2,
        frame,
        now: NOW,
        filter,
        ..View::default()
    };
    cells(&render_graph(swarm, &view, 160, H, None, NO_SESSIONS))
}

/// The cells of `xs` × `ys` that differ.
fn diff(
    want: &[Vec<Shown>],
    got: &[Vec<Shown>],
    xs: std::ops::Range<usize>,
    ys: &[usize],
) -> Vec<String> {
    let mut out = vec![];
    for &y in ys {
        for x in xs.clone() {
            if want[y][x] != got[y][x] {
                out.push(format!(
                    "({x},{y}) want {:?} got {:?}",
                    want[y][x], got[y][x]
                ));
            }
        }
    }
    out
}

/// The orchestrator cards: the new ones (tabs 5-8, rows 23-42) whole; the earlier four keep
/// what the app drew before turn 8 (the app gives a name 4 more cells than the design's 2a),
/// so only their state lines are compared.
const ORCH_X: std::ops::Range<usize> = 2..30;

fn orch_rows(last: usize) -> Vec<usize> {
    [5, 10, 15, 20].into_iter().chain(23..last).collect()
}

fn check(name: &str, d: Vec<String>) {
    assert!(
        d.is_empty(),
        "{name}: {} cells differ, first:\n{}",
        d.len(),
        d[..d.len().min(5)].join("\n")
    );
}

#[test]
fn turn_8_2a_orchestrators_and_background_card() {
    let eight = fixture("design_8.json");
    let swarm = states_swarm(&eight["data"]);
    let bg = format!("{BG_CARD}w1:p5");
    for (key, panes) in eight["frames"].as_object().unwrap() {
        let got = graph(
            &swarm,
            key.parse().unwrap(),
            ("w1:p5", Some(&bg), None),
            Filter::All,
        );
        let want = golden(&panes["A"]);
        let mut d = diff(&want, &got, ORCH_X, &orch_rows(43));
        d.extend(diff(&want, &got, 36..86, &[3, 4, 5, 6, 7]));
        check(&format!("2a A frame {key}"), d);
    }
}

#[test]
fn turn_8_2a_waiting_agent_card() {
    let eight = fixture("design_8.json");
    let swarm = states_swarm(&eight["data"]);
    for (key, panes) in eight["frames"].as_object().unwrap() {
        let got = graph(
            &swarm,
            key.parse().unwrap(),
            ("w1:p4", Some("p214"), Some("n1")),
            Filter::All,
        );
        let want = golden(&panes["B"]);
        let mut d = diff(&want, &got, ORCH_X, &orch_rows(43));
        d.extend(diff(&want, &got, 93..137, &(3..13).collect::<Vec<_>>()));
        check(&format!("2a B frame {key}"), d);
    }
}

#[test]
fn turn_8_running_filter_keeps_waiting_and_drops_stale() {
    let eight = fixture("design_8.json");
    let swarm = states_swarm(&eight["data"]);
    for (key, panes) in eight["frames"].as_object().unwrap() {
        let sel = ("w1:p4", Some("p214"), Some("n1"));
        let got = graph(&swarm, key.parse().unwrap(), sel, Filter::Running);
        // 6 cards: the stale ones are gone
        let d = diff(&golden(&panes["R"]), &got, ORCH_X, &orch_rows(33));
        check(&format!("2a R frame {key}"), d);
    }
}

#[test]
fn the_background_card_takes_the_wide_thread_column() {
    // test_states.py::test_the_background_card_takes_the_wide_thread_column (turn 7's wider
    // cards): the background card is as wide as the thread cards below it
    let mut swarm = states_swarm(&fixture("design_8.json")["data"]);
    swarm.orchestrators[3].bg = vec![BgTask {
        id: "x".into(),
        kind: "Bash".into(),
        description: "a long description ".repeat(4),
        started: Some(NOW - 60.0),
    }];
    let view = View {
        orch: Some("w1:p4".into()),
        thread: Some("p214".into()),
        agent: Some("n1".into()),
        focus: 2,
        now: NOW,
        ..View::default()
    };
    for w in [160, 240, 320] {
        let text = render_graph(&swarm, &view, w, H, None, NO_SESSIONS).text();
        // the first `│` after the word, in characters
        let right = |word: &str| {
            let row = text.lines().find(|r| r.contains(word)).unwrap();
            let at = row[..row.find(word).unwrap()].chars().count();
            at + row.chars().skip(at).position(|c| c == '│').unwrap()
        };
        assert_eq!(right("background"), right("PR #212"), "{w}");
    }
}
