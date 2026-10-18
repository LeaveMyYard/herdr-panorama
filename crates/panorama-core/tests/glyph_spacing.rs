//! No two status glyphs in adjacent cells and no half-circle spinner, in the docked panel and
//! the narrow layout: Python's tests/test_glyph_spacing.py `screens()` for the views P3 draws
//! (the wide graph's widths, 120 and up, come with it in P4).

mod support;

use panorama_core::model::Swarm;
use panorama_core::view::{Screen, View, render_dock, render_narrow};
use support::design::design_swarm;
use support::glyphs::adjacent;
use support::{NO_SESSIONS, fixture, wire_swarm};

/// tests/design_deep.py `NOW` and `TID`; tests/demo.py `NOW`.
const DEEP_NOW: f64 = 1_800_000_000.0;
const DEEP_TID: &str = "x:deep";

fn now_of(rel: &str) -> f64 {
    fixture(rel)["now"].as_f64().unwrap()
}

fn docked(mut sw: Swarm) -> Swarm {
    sw.docked = Some("w1:p3".into());
    sw
}

/// (name, screen) for every P3 view of `screens()`, frames 0–3.
fn screens() -> Vec<(String, Screen)> {
    let demo_now = now_of("inputs/swarm-demo.json");
    let mut out = vec![];
    for frame in 0..4 {
        let f = f64::from(frame);
        for w in [46, 60] {
            let v = View {
                orch: Some("w1:p3".into()),
                frame: f,
                ..View::default()
            };
            let sw = docked(design_swarm(&fixture("design_data.json")));
            out.push((
                format!("dock {w} f{frame}"),
                render_dock(&sw, &v, w, 45, None, NO_SESSIONS),
            ));
        }
        let v = View {
            orch: Some("w1:p3".into()),
            frame: f,
            now: DEEP_NOW,
            ..View::default()
        };
        let sw = docked(wire_swarm("inputs/swarm-deep.json", None));
        out.push((
            format!("deep dock f{frame}"),
            render_dock(&sw, &v, 46, 45, None, NO_SESSIONS),
        ));
        let v = View {
            orch: Some("w1:p1".into()),
            thread: Some("ref:#42".into()),
            agent: Some("a2f81c0".into()),
            focus: 2,
            now: demo_now,
            frame: f,
            ..View::default()
        };
        let sw = wire_swarm("inputs/swarm-demo.json", None);
        out.push((
            format!("demo 70 f{frame}"),
            render_narrow(&sw, &v, 70, 45, None, NO_SESSIONS),
        ));
        let v = View {
            orch: Some("w1:p3".into()),
            thread: Some(DEEP_TID.into()),
            agent: Some("c1".into()),
            focus: 5,
            now: DEEP_NOW,
            frame: f,
            ..View::default()
        };
        let sw = wire_swarm("inputs/swarm-deep.json", None);
        out.push((
            format!("deep 70 f{frame}"),
            render_narrow(&sw, &v, 70, 45, None, NO_SESSIONS),
        ));
    }
    out
}

#[test]
fn no_two_glyphs_touch() {
    // test_no_two_glyphs_touch[dock 46/60, deep dock, demo 70, deep 70 × f0–f3]
    for (name, s) in screens() {
        assert!(
            s.text()
                .chars()
                .any(|c| support::glyphs::GLYPHS.contains(c)),
            "{name}: no glyph"
        );
        assert_eq!(adjacent(&s), vec![], "{name}");
    }
}

#[test]
fn no_half_circle_glyphs_anywhere() {
    // test_no_half_circle_glyphs_anywhere[the same screens]
    for (name, s) in screens() {
        assert!(!s.text().chars().any(|c| "◐◑◒◓".contains(c)), "{name}");
    }
}
