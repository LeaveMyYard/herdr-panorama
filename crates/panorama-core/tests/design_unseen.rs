//! The unseen pulse on the 2a graph (design turn 6, `design_6.json`) and its glide between
//! frames (`design_smooth.json`), against the design's renders: every cell the design's
//! pulse changes (marks vs none) changes in the graph to the same character, colour and
//! weight, and no other; compared where the design and the graph draw the same character
//! without marks (their sample texts differ in a few tags). Python's tests/test_unseen.py
//! `test_2a_pulses_the_same_cells_in_the_same_colours_as_the_design` and
//! tests/test_smooth.py `test_2a_glides_like_the_design_between_frames`.
//!
//! The graph runs a selected card's pulse over its selection colour, capped (the operator's
//! report: the current card was lost under the pulse), where the design runs every pulse over
//! the plain border: a deliberate difference. Python set it aside by patching the cap out;
//! here the selected cards' borders must pulse in the same cells, and are set aside only for
//! their colour.

mod support;

use panorama_core::model::Status;
use panorama_core::view::palette::{FAINT, status};
use panorama_core::view::target::AgentRef;
use panorama_core::view::{Marks, Screen, Target, Unseen, View, render_graph};
use std::collections::BTreeMap;
use support::design::design_swarm;
use support::{NO_SESSIONS, Shown, cells, fixture, golden};

/// The design's sample marks (test_unseen.py `MARKS`).
fn marks() -> Marks {
    [
        ("b3", Unseen::Fail),
        ("c1", Unseen::Wait),
        ("a5", Unseen::Done),
        ("i3", Unseen::Wait),
        ("k2", Unseen::Done),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect()
}

fn graph(frame: f64, unseen: Marks) -> Screen {
    let view = View {
        orch: Some("w1:p1".into()),
        thread: Some("t595".into()),
        agent: Some("a2".into()),
        focus: 2,
        frame,
        unseen,
        ..View::default()
    };
    render_graph(
        &design_swarm(&fixture("design_data.json")),
        &view,
        160,
        45,
        None,
        NO_SESSIONS,
    )
}

/// Whether `(x, y)` is on the border of a selected card (the orchestrator, thread and agent
/// selected): the card's area as its click target spans it.
fn on_selected_border(s: &Screen, x: i32, y: i32) -> bool {
    let selected = [
        Target::Orch("w1:p1".into()),
        Target::Thread {
            pane: "w1:p1".into(),
            thread: "t595".into(),
        },
        Target::Agent(AgentRef {
            pane: "w1:p1".into(),
            thread: "t595".into(),
            agent: "a2".into(),
        }),
    ];
    selected.iter().any(|t| {
        // a card's rows (its pods are 3-cell targets of their own)
        let rows: Vec<_> = s
            .targets
            .iter()
            .filter(|h| &h.target == t && h.x1 - h.x0 > 3)
            .collect();
        let (Some(top), Some(bottom)) = (
            rows.iter().map(|h| h.y).min(),
            rows.iter().map(|h| h.y).max(),
        ) else {
            return false;
        };
        rows.iter().any(|h| {
            h.y == y
                && (x == h.x0
                    || x == h.x1 - 1
                    || ((y == top || y == bottom) && (h.x0..h.x1).contains(&x)))
        })
    })
}

type Cells = BTreeMap<(usize, usize), Shown>;

/// The pulsed cells, design and graph, where both draw the same character without marks;
/// the pulsed cells set aside (a selected card's border).
struct Pulse {
    want: Cells,
    got: Cells,
    selected: Vec<(usize, usize)>,
    /// The cells compared, without marks: (design, graph).
    off: Vec<(Shown, Shown)>,
}

fn pulse(design: &serde_json::Value, frame: f64) -> Pulse {
    let (d_on, d_off) = (golden(&design["G"]), golden(&design["N"]));
    let off_screen = graph(frame, Marks::new());
    let (a_on, a_off) = (cells(&graph(frame, marks())), cells(&off_screen));
    let (mut want, mut got, mut selected, mut off) = (Cells::new(), Cells::new(), vec![], vec![]);
    for y in 0..45 {
        for x in 0..160 {
            if d_off[y][x].ch != a_off[y][x].ch {
                continue;
            }
            let (d, a) = (d_on[y][x] != d_off[y][x], a_on[y][x] != a_off[y][x]);
            if on_selected_border(&off_screen, x as i32, y as i32) {
                assert_eq!(
                    d, a,
                    "({x},{y}): a selected border pulses where the design's does"
                );
                // only its colour is set aside: the character and the weight are the design's
                let (w, g) = (&d_on[y][x], &a_on[y][x]);
                assert_eq!(
                    (w.ch, w.bold),
                    (g.ch, g.bold),
                    "({x},{y}): a selected border"
                );
                if d {
                    selected.push((x, y));
                }
                continue;
            }
            off.push((d_off[y][x].clone(), a_off[y][x].clone()));
            if d {
                want.insert((x, y), d_on[y][x].clone());
            }
            if a {
                got.insert((x, y), a_on[y][x].clone());
            }
        }
    }
    Pulse {
        want,
        got,
        selected,
        off,
    }
}

fn assert_pulse(name: &str, p: &Pulse) {
    assert!(p.off.len() > 6000, "{name}: {} cells compared", p.off.len());
    let pulsed = p.want.len() + p.selected.len();
    assert!(pulsed > 400, "{name}: {pulsed} cells pulse");
    assert!(
        !p.selected.is_empty(),
        "{name}: a selected card's border pulses"
    );
    let bad: Vec<String> = p
        .want
        .keys()
        .chain(p.got.keys())
        .filter(|k| p.want.get(k) != p.got.get(k))
        .take(5)
        .map(|k| format!("{k:?} want {:?} got {:?}", p.want.get(k), p.got.get(k)))
        .collect();
    assert!(
        bad.is_empty(),
        "{name}: pulsed cells differ:\n{}",
        bad.join("\n")
    );
}

#[test]
fn design_6_the_2a_graph_pulses_like_the_design() {
    let six = fixture("design_6.json");
    for (key, design) in six["frames"].as_object().unwrap() {
        assert_pulse(
            &format!("6 frame {key}"),
            &pulse(design, key.parse().unwrap()),
        );
    }
}

#[test]
fn design_smooth_the_2a_graph_glides_like_the_design() {
    let smooth = fixture("design_smooth.json");
    let palette: Vec<Option<String>> = [
        Status::Queued,
        Status::Running,
        Status::Reviewing,
        Status::Waiting,
        Status::Done,
        Status::Failed,
        Status::Cancelled,
        Status::Inferred,
        Status::Lost,
        Status::Background,
    ]
    .iter()
    .map(|s| Some(status(s).to_string()))
    .chain([Some(FAINT.to_string())])
    .collect();
    for (key, design) in smooth["frames"].as_object().unwrap() {
        let f: f64 = key.parse().unwrap();
        let p = pulse(design, f);
        assert_pulse(&format!("smooth frame {key}"), &p);
        // the blinking left edges (┃ in an attention colour, no pulse) glide the same way
        let edges: Vec<&(Shown, Shown)> = p.off.iter().filter(|(d, _)| d.ch == '┃').collect();
        assert!(
            !edges.is_empty() && edges.iter().all(|(d, a)| d == a),
            "smooth frame {key}: edges"
        );
        let fading = edges.iter().any(|(d, _)| !palette.contains(&d.fg));
        if (1.0..2.0).contains(&(f % 4.0)) && f % 4.0 > 1.0 || f % 4.0 > 3.0 {
            assert!(
                fading,
                "smooth frame {key}: mid-fade, some edge is between on and off"
            );
        }
    }
}
