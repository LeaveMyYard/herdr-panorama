//! The unseen pulse on the 2a graph (design turn 6, `design_6.json`) and its glide between
//! frames (`design_smooth.json`), at the design's frames and sample marks. Python's
//! tests/test_unseen.py `test_2a_pulses_the_same_cells_in_the_same_colours_as_the_design`
//! and tests/test_smooth.py `test_2a_glides_like_the_design_between_frames`.
//!
//! The designs draw the thread column the uniform tree removed (plan `uniform-agent-tree.md`
//! U4, D5): the pages, marks and none, are compared with Rust's own reviewed renders,
//! `tests/fixtures/rust/screens/design-{6,smooth}-wide.json` (`PANORAMA_BLESS=1`), and the
//! pulse's rules are checked on them: it changes only card borders, a selected card's too,
//! and the blinking left edges glide through mid-fade colours without it.
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
use support::design::design_swarm;
use support::owned::check_owned;
use support::{NO_SESSIONS, Shown, cells, fixture};

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
        agent: Some("a2".into()),
        focus: 1,
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

/// Whether `(x, y)` is on the border of a card (`selected`: the selected root's or agent's):
/// the card's area as its click target spans it.
fn on_border(s: &Screen, x: i32, y: i32, selected: bool) -> bool {
    let chosen = [
        Target::Orch("w1:p1".into()),
        Target::Agent(AgentRef {
            pane: "w1:p1".into(),
            agent: "a2".into(),
        }),
    ];
    let mut cards: Vec<&Target> = s.targets.iter().map(|h| &h.target).collect();
    cards.dedup();
    cards
        .into_iter()
        .filter(|t| t.is_card() && (!selected || chosen.contains(t)))
        .any(|t| {
            let rows: Vec<_> = s.targets.iter().filter(|h| &h.target == t).collect();
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

/// The frame's pages with and without the marks, against Rust's renders (`file`); the
/// cells the pulse changes, and those of them on a selected card's border.
fn pulse(file: &str, key: &str) -> (Vec<(usize, usize)>, usize, Vec<Vec<Shown>>) {
    let f: f64 = key.parse().unwrap();
    let (on, off) = (graph(f, marks()), graph(f, Marks::new()));
    check_owned(file, &format!("marks frame {key}"), &on);
    check_owned(file, &format!("none frame {key}"), &off);
    let (a_on, a_off) = (cells(&on), cells(&off));
    let mut pulsed = vec![];
    let mut selected = 0;
    for y in 0..45 {
        for x in 0..160 {
            if a_on[y][x] == a_off[y][x] {
                continue;
            }
            let (xi, yi) = (x as i32, y as i32);
            assert!(
                on_border(&off, xi, yi, false),
                "{file} {key}: ({x},{y}) pulses off a card's border"
            );
            assert_eq!(a_on[y][x].ch, a_off[y][x].ch, "({x},{y}): the border's own");
            selected += usize::from(on_border(&off, xi, yi, true));
            pulsed.push((x, y));
        }
    }
    (pulsed, selected, a_off)
}

#[test]
fn design_6_the_2a_graph_pulses_card_borders() {
    let six = fixture("design_6.json");
    for key in six["frames"].as_object().unwrap().keys() {
        let (pulsed, selected, _) = pulse("design-6-wide.json", key);
        assert!(
            pulsed.len() > 100,
            "6 frame {key}: {} cells pulse",
            pulsed.len()
        );
        assert!(
            selected > 0,
            "6 frame {key}: a selected card's border pulses"
        );
    }
}

#[test]
fn design_smooth_the_2a_graph_glides_between_frames() {
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
    for key in smooth["frames"].as_object().unwrap().keys() {
        let f: f64 = key.parse().unwrap();
        let (pulsed, _, off) = pulse("design-smooth-wide.json", key);
        assert!(!pulsed.is_empty(), "smooth frame {key}: the marks pulse");
        // the blinking left edges (┃ in an attention colour) are not the pulse's
        let edges: Vec<&Shown> = off.iter().flatten().filter(|c| c.ch == '┃').collect();
        assert!(!edges.is_empty(), "smooth frame {key}: edges");
        let fading = edges.iter().any(|c| !palette.contains(&c.fg));
        if (1.0..2.0).contains(&(f % 4.0)) && f % 4.0 > 1.0 || f % 4.0 > 3.0 {
            assert!(
                fading,
                "smooth frame {key}: mid-fade, some edge is between on and off"
            );
        }
    }
}
