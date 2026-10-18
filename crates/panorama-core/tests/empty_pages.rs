//! The page shown instead of a swarm view when herdr is not reachable, against the
//! app-screen golden captured with herdr down (`screens/scenario-demo-failures.json`): the
//! graph's chrome (header with filters, totals and clock; wide legend; keys).

mod support;

use panorama_core::model::Swarm;
use panorama_core::view::{View, render_dock};
use support::{assert_same, cells, fixture, golden};

#[test]
fn herdr_down_shows_the_retry_page() {
    let file = fixture("screens/scenario-demo-failures.json");
    let run = &file["runs"][0];
    assert_eq!(run["env"]["FAKE_HERDR_FAIL"], "api snapshot");
    // the model golden of the same run: `panorama snapshot --json` with herdr down
    let mut snap = fixture("model/demo.json")["snapshots"]["+0m herdr down"].clone();
    snap.as_object_mut().unwrap().remove("counts");
    let swarm: Swarm = serde_json::from_value(snap).unwrap();
    assert!(!swarm.herdr_ok);
    let view = View {
        now: run["now"].as_f64().unwrap(),
        ..View::default()
    }; // TZ=UTC
    let got = render_dock(&swarm, &view, 160, 45, None);
    assert_same(
        "herdr down",
        &golden(&file["screens"]["herdr down"]),
        &cells(&got),
    );
}

#[test]
fn no_sessions_at_panel_width() {
    // the rows Python's render.render draws for this (TZ=UTC; read from it once: no golden
    // holds a 46-column empty page without the panel's notice over it). The words are the
    // caller's: the Claude Code panel passes Python's.
    let view = View {
        now: 1791972600.0,
        no_sessions: "No Claude Code sessions in herdr".into(),
        ..View::default()
    };
    let s = render_dock(&Swarm::default(), &view, 46, 45, None);
    let text = s.text();
    let rows: Vec<&str> = text.lines().collect();
    assert_eq!(rows[0], " herdr panorama    all   running   needs10:10");
    assert_eq!(rows[2], "  No Claude Code sessions in herdr");
    assert_eq!(rows[42], " ✓ done ⠋ run » rev ○ queue ? ask ! block ✗ fa");
    assert_eq!(rows[44], " ↑↓ move   → open   g pane   a answer   f filt");
}
