//! The app's model on the demo and design 9 swarms (captured in wire form): the halves of
//! tests/test_tui.py and tests/test_dock_panel.py cases that are the model's (their keys and
//! effects join with the app's input, P4 PR 8b).

use super::*;
use crate::ui::selection::ARM;
use panorama_core::view::nav::nav;
use serde_json::Value;
use std::time::Duration;

fn wire(name: &str) -> Swarm {
    let path = format!(
        "{}/tests/fixtures/inputs/swarm-{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    serde_json::from_value(v["swarm"].clone()).unwrap()
}

fn demo() -> Swarm {
    wire("demo")
}

/// `(focus, orch, thread, agent)` (test_tui.py `sel`).
fn sel(app: &App) -> (usize, Option<&str>, Option<&str>, Option<&str>) {
    let v = &app.view;
    (
        v.focus,
        v.orch.as_deref(),
        v.thread.as_deref(),
        v.agent.as_deref(),
    )
}

/// An app opened from `pane` that got the demo at `t0`, its keys armed again after.
fn loaded(pane: &str) -> (App, Instant) {
    let t0 = Instant::now();
    let mut app = App::new(Some(pane.into()), false, t0);
    app.apply(demo(), Marks::new(), t0);
    app.before_paint(160, 45);
    (app, t0)
}

#[test]
fn the_first_selection_follows_the_invoking_pane() {
    // test_tui.py::test_initial_selection_follows_the_invoking_pane
    let (app, t0) = loaded("w1:p2");
    assert_eq!(
        sel(&app),
        (0, Some("w1:p2"), Some("wf:wf_search"), Some("j1a2b3c"))
    );
    assert!(!app.armed(t0) && app.armed(t0 + ARM)); // the first paint arms the keys later
    // a pane that is not an orchestrator: the first row
    let mut app = App::new(Some("w9:p9".into()), false, t0);
    app.apply(demo(), Marks::new(), t0);
    assert_eq!(app.view.orch.as_deref(), Some("w1:p1"));
}

#[test]
fn the_cursor_tracks_identity_when_a_refresh_reorders() {
    // test_tui.py::test_cursor_tracks_identity_when_a_refresh_reorders
    let (mut app, t0) = loaded("w1:p2");
    let later = t0 + Duration::from_secs(5);
    app.armed_at = t0;
    let mut sw = demo();
    sw.orchestrators.reverse(); // a new tab order arrives in the background
    app.apply(sw, Marks::new(), later);
    assert_eq!(
        sel(&app),
        (0, Some("w1:p2"), Some("wf:wf_search"), Some("j1a2b3c"))
    );
    assert!(app.armed(later)); // the same card: not armed again
    assert_eq!(nav(&app.swarm, &app.view)[0][0], "w1:p4"); // the arrows see the new order
}

#[test]
fn a_vanished_target_parks_the_cursor() {
    // test_tui.py::test_a_vanished_target_parks_the_cursor
    let (mut app, t0) = loaded("w1:p1");
    let mut sw = demo();
    sw.orchestrators.retain(|o| o.pane_id != "w1:p1");
    app.apply(sw, Marks::new(), t0 + Duration::from_secs(5));
    assert_eq!(sel(&app), (0, None, None, None));
    assert_eq!(app.parked(), Some("w1:p1")); // where the arrows resume from
}

#[test]
fn a_background_change_under_the_cursor_arms_the_keys_again() {
    // test_tui.py::test_action_keys_are_ignored_right_after_a_background_change and
    // ::test_m5_a_new_inbox_question_arms_the_answer_key, the model's half
    let (mut app, t0) = loaded("w1:p1");
    let later = t0 + Duration::from_secs(5);
    let mut sw = demo();
    let a = &mut sw.orchestrators[0].threads[0].agents[1];
    assert_eq!(app.view.agent.as_deref(), Some(a.id.as_str()));
    a.status = panorama_core::model::Status::Done; // the selected agent changed under the cursor
    app.apply(sw, Marks::new(), later);
    assert!(!app.armed(later) && app.armed(later + ARM));
    let (mut app, t0) = loaded("w1:p2");
    let later = t0 + Duration::from_secs(5);
    let mut sw = demo();
    sw.orchestrators[1].inbox = vec![9]; // `a` would now open the inbox
    app.apply(sw, Marks::new(), later);
    assert!(!app.armed(later));
}

#[test]
fn a_failed_refresh_keeps_the_last_model_and_says_why() {
    // test_tui.py::test_l1_a_failing_collect_keeps_the_last_model_and_says_so and
    // ::test_refresh_errors_never_kill_the_view, the model's half
    let (mut app, t0) = loaded("w1:p1");
    app.failed(&format!("RuntimeError: {}", "bad record ".repeat(40)));
    assert_eq!(app.swarm, demo());
    let why = app.stale().unwrap();
    assert!(why.starts_with("RuntimeError: bad record") && why.chars().count() == 200);
    app.apply(demo(), Marks::new(), t0); // the next good one clears it
    assert_eq!(app.stale(), None);
    let mut sw = demo();
    sw.errors = vec!["unreadable record".into()];
    app.apply(sw, Marks::new(), t0);
    assert_eq!(app.stale(), Some("unreadable record"));
    // failing before any model: nothing selected, still nothing loaded
    let mut app = App::new(None, false, t0);
    app.failed("parse failure");
    assert!(!app.loaded && app.stale() == Some("parse failure"));
}

fn design9(docked: bool) -> Swarm {
    let mut sw = wire("design9");
    sw.docked = docked.then(|| "w1:p3".into());
    sw
}

#[test]
fn docked_and_narrow_is_the_docked_panel_with_one_column() {
    // test_dock_panel.py::test_docked_view_uses_the_panel_and_keeps_one_column, the model's half
    let t0 = Instant::now();
    let mut app = App::new(Some("w1:p3".into()), true, t0);
    app.apply(design9(true), Marks::new(), t0);
    app.apply(design9(true), Marks::new(), t0);
    assert!(!app.dock); // no width before the first paint: no layout decided yet
    app.before_paint(46, 45);
    assert!(app.dock && app.view.focus == 0);
    assert!(app.view.zoom_hint);
    app.view.focus = 1; // a key cannot leave the list's one column: the next paint says so
    app.before_paint(46, 45);
    assert_eq!(app.view.focus, 0);
    // wide, or not docked: the graph
    app.before_paint(160, 45);
    assert!(!app.dock);
    app.apply(design9(false), Marks::new(), t0);
    app.before_paint(46, 45);
    assert!(!app.dock && !app.view.zoom_hint);
    // a popup has no zoom
    let mut popup = App::new(Some("w1:p3".into()), false, t0);
    popup.apply(design9(true), Marks::new(), t0);
    popup.before_paint(46, 45);
    assert!(popup.dock && !popup.view.zoom_hint);
}

#[test]
fn a_refresh_that_docks_the_view_arms_the_keys_again() {
    // test_dock_panel.py::test_r11_a_refresh_that_docks_the_view_rearms_the_keys
    let t0 = Instant::now();
    let mut app = App::new(Some("w1:p3".into()), true, t0);
    app.apply(design9(false), Marks::new(), t0);
    app.before_paint(46, 45);
    app.view.focus = 1; // on a thread
    assert!(!app.dock);
    let later = t0 + Duration::from_secs(5);
    app.apply(design9(true), Marks::new(), later); // panorama docked next to tab 3
    assert!(app.dock && app.view.focus == 0);
    assert!(app.armed_at >= later + ARM);
}

#[test]
fn a_paint_keeps_each_columns_scroll_offset() {
    let (mut app, _) = loaded("w1:p1");
    let mut s = Screen::new(10, 10);
    s.columns.push(panorama_core::view::ScrollRegion {
        x0: 0,
        x1: 5,
        y0: 0,
        y1: 10,
        column: 1,
        offset: 7,
    });
    app.painted(&s);
    assert_eq!(app.view.offsets.get(&1), Some(&7));
}

#[test]
fn a_pane_with_no_size_is_not_painted_and_keeps_its_layout() {
    // Python's repaint returns before deciding anything at width or height 0
    let t0 = Instant::now();
    let mut app = App::new(Some("w1:p3".into()), true, t0);
    app.apply(design9(true), Marks::new(), t0);
    assert!(app.before_paint(46, 45) && app.dock);
    assert!(!app.before_paint(0, 45) && app.dock && app.width == 46);
    assert!(!app.before_paint(46, 0) && app.dock);
}
