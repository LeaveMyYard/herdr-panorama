//! The panel's selection and focus rules (tests/test_panel_sync.py and test_panel_filter.py's
//! cases, on the model alone).

use super::*;
use crate::proto::state::State;
use serde_json::{Value, json};

fn state(extra: Value) -> State {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/inputs/swarm-design9.json"
    );
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut st = json!({ "swarm": fixture["swarm"] });
    st.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    State::read(st.as_object().unwrap()).unwrap()
}

/// A state now.
fn feed(p: &mut Panel, st: State) {
    p.on_state(st, std::time::Instant::now());
}

fn panel() -> Panel {
    Panel::new(Some("w1:p92".into()), Some("w1:t2".into()))
}

#[test]
fn the_docked_entry_is_this_tabs_and_the_selected_row_the_shown_one() {
    let mut p = panel();
    feed(&mut p, state(json!({"visible_tabs": ["w1:t4"]})));
    assert_eq!(p.swarm.docked.as_deref(), Some("w1:p2"));
    assert_eq!(p.view.orch.as_deref(), Some("w1:p4"));
    assert!(!p.tab_shown);
    feed(&mut p, state(json!({"visible_tabs": ["w1:t2"]})));
    assert_eq!(p.view.orch.as_deref(), Some("w1:p2"));
    assert!(p.tab_shown);
    // unknown visibility: shown, and the focused pane decides
    feed(
        &mut p,
        state(json!({"visible_tabs": null, "focused_pane": "w1:p3"})),
    );
    assert!(p.tab_shown);
    assert_eq!(p.view.orch.as_deref(), Some("w1:p3"));
}

#[test]
fn a_filter_that_hides_the_shown_one_selects_nothing_until_it_shows_it_again() {
    let mut p = panel();
    feed(&mut p, state(json!({"visible_tabs": ["w1:t5"]}))); // Docs Site: all done
    assert_eq!(p.view.orch.as_deref(), Some("w1:p5"));
    feed(
        &mut p,
        state(json!({"visible_tabs": ["w1:t5"], "filter": "running"})),
    );
    assert_eq!(p.view.filter, Filter::Running);
    assert_eq!(p.view.orch, None);
    assert_eq!(p.parked(), Some("w1:p5"));
    feed(
        &mut p,
        state(json!({"visible_tabs": ["w1:t5"], "filter": "all"})),
    );
    assert_eq!(p.view.orch.as_deref(), Some("w1:p5"));
}

#[test]
fn the_focus_look_follows_herdrs_focus_and_the_terminals() {
    let mut p = panel();
    feed(&mut p, state(json!({"focused_pane": "w1:p2"})));
    assert!(!p.pane_focused);
    feed(&mut p, state(json!({"focused_pane": "w1:p92"})));
    assert!(p.pane_focused);
    p.on_terminal_focus(false);
    assert!(!p.pane_focused);
}

#[test]
fn a_tab_without_an_orchestrator_shows_the_graph_with_the_shown_one_opened() {
    use crate::panel::paint::Frame;
    let mut p = Panel::new(Some("w1:p99".into()), Some("w1:t9".into()));
    p.on_connected(true);
    feed(&mut p, state(json!({"visible_tabs": ["w1:t1"]})));
    assert_eq!(p.swarm.docked, None);
    // Python's initial selection: the shown orchestrator, its first thread and agent
    assert_eq!(p.view.orch.as_deref(), Some("w1:p1"));
    assert!(p.view.thread.is_some() && p.view.agent.is_some());
    let f = Frame {
        now: 0.0,
        frame: 0.0,
        utc_offset: 0,
        theme: None,
        settings: Default::default(),
        open_key: ("pfx+spc".into(), "pfx+spc".into()),
    };
    let text = p.paint(46, 45, &f).text();
    assert!(text.starts_with(" herdr panorama"), "{text}");
    // from 120 columns the wide graph, as Python's render draws it there: the legend's tiles
    // and the wide keys
    let text = p.paint(160, 45, &f).text();
    let rows: Vec<&str> = text.lines().collect();
    assert!(rows[42].starts_with("  ✓  done   ✓? done?"), "{}", rows[42]);
    assert!(
        rows[44].starts_with(" ↑↓ move   → open   ← back   enter details"),
        "{}",
        rows[44]
    );
    // docked when its tab has one
    let mut p = panel();
    p.on_connected(true);
    feed(&mut p, state(json!({"visible_tabs": ["w1:t1"]})));
    assert!(p.paint(46, 45, &f).text().starts_with(" SWARM"));
}

#[test]
fn an_empty_focused_pane_changes_nothing() {
    let mut p = panel();
    p.on_terminal_focus(false);
    p.herdr_focus("");
    assert!(!p.pane_focused);
    p.herdr_focus("w1:p92");
    assert!(p.pane_focused);
    p.herdr_focus("");
    assert!(p.pane_focused);
}

#[test]
fn the_docked_view_needs_a_pane_narrower_than_the_wide_layout() {
    use crate::panel::paint::Frame;
    let f = Frame {
        now: 0.0,
        frame: 0.0,
        utc_offset: 0,
        theme: None,
        settings: Default::default(),
        open_key: ("pfx+spc".into(), "pfx+spc".into()),
    };
    let mut p = panel();
    p.on_connected(true);
    feed(&mut p, state(json!({"visible_tabs": ["w1:t1"]})));
    assert!(p.paint(119, 45, &f).text().starts_with(" SWARM"));
    // Python draws the wide graph from 120 columns (P4); until then, the narrow layout
    assert!(p.paint(120, 45, &f).text().starts_with(" herdr panorama"));
}

fn frame() -> crate::panel::paint::Frame {
    crate::panel::paint::Frame {
        now: 0.0,
        frame: 0.0,
        utc_offset: 0,
        theme: None,
        settings: Default::default(),
        open_key: ("pfx+spc".into(), "pfx+spc".into()),
    }
}

/// An empty row of the docked panel (test_panel_sync.py reads `c[30][20]`).
fn empty_row_bg(p: &mut Panel) -> Option<panorama_core::view::Rgb> {
    p.paint(46, 45, &frame()).cell(20, 30).bg
}

#[test]
fn an_arrow_move_waiting_for_its_switch_is_kept() {
    // test_panel_sync.py::test_an_arrow_move_waiting_for_its_switch_is_kept
    let mut p = panel();
    feed(&mut p, state(json!({"visible_tabs": ["w1:t2"]})));
    p.view.orch = Some("w1:p3".into()); // the arrow's move, its switch still pending
    feed(&mut p, state(json!({"visible_tabs": ["w1:t2"]})));
    assert_eq!(p.view.orch.as_deref(), Some("w1:p3"));
    feed(&mut p, state(json!({"visible_tabs": ["w1:t3"]}))); // the switch happened
    assert_eq!(p.view.orch.as_deref(), Some("w1:p3"));
}

#[test]
fn the_switching_panel_keeps_its_look_until_it_is_hidden() {
    // test_panel_sync.py::test_the_switching_panel_keeps_its_look_until_it_is_hidden
    use panorama_core::view::theme::FIXED;
    let mut p = panel();
    p.on_connected(true);
    feed(
        &mut p,
        state(json!({"focused_pane": "w1:p92", "visible_tabs": ["w1:t2"]})),
    );
    assert!(p.pane_focused);
    assert!(!p.switch("w1:p4".into(), true).is_empty()); // under way: the core names the target
    feed(
        &mut p,
        state(json!({"focused_pane": "w1:p94", "visible_tabs": ["w1:t2"]})),
    );
    assert!(p.pane_focused);
    assert_eq!(empty_row_bg(&mut p), Some(FIXED.0));
    p.switched(None, false, std::time::Instant::now()); // done: the tab is shown elsewhere now
    assert!(!p.pane_focused);
}

#[test]
fn a_hidden_target_paints_focused_before_it_is_shown() {
    // test_panel_sync.py::test_a_hidden_target_paints_focused_before_it_is_shown
    use panorama_core::view::theme::FIXED;
    let mut p = Panel::new(Some("w1:p94".into()), Some("w1:t4".into()));
    p.on_connected(true);
    feed(
        &mut p,
        state(json!({"focused_pane": "w1:p92", "visible_tabs": ["w1:t2"]})),
    );
    assert_eq!(empty_row_bg(&mut p), Some(FIXED.1));
    // announced, its tab not shown yet
    feed(
        &mut p,
        state(json!({"focused_pane": "w1:p94", "visible_tabs": ["w1:t2"]})),
    );
    assert!(!p.tab_shown);
    assert_eq!(empty_row_bg(&mut p), Some(FIXED.0));
}

#[test]
fn the_shown_orchestrator_is_selected_again_when_the_filter_shows_it_again() {
    // test_panel_filter.py::test_the_shown_orchestrator_is_selected_again_when_the_filter_shows_it_again:
    // tab 3's panel under "needs"; its questions answered (hidden), then a new one arrives
    let mut p = Panel::new(Some("w1:p93".into()), Some("w1:t3".into()));
    let with_inbox = |inbox: Value| {
        let mut st = state(json!({"visible_tabs": ["w1:t3"], "filter": "needs"}));
        let o = st
            .swarm
            .orchestrators
            .iter_mut()
            .find(|o| o.pane_id == "w1:p3")
            .unwrap();
        o.inbox = serde_json::from_value(inbox).unwrap();
        for a in o.threads.iter_mut().flat_map(|t| t.agents.iter_mut()) {
            a.question_ids.clear();
            if a.status == panorama_core::model::Status::Waiting {
                a.status = panorama_core::model::Status::Running;
            }
        }
        st
    };
    feed(&mut p, with_inbox(json!([7])));
    assert_eq!(p.view.orch.as_deref(), Some("w1:p3"));
    feed(&mut p, with_inbox(json!([])));
    assert_eq!(p.view.orch, None);
    feed(&mut p, with_inbox(json!([8])));
    assert_eq!(p.view.orch.as_deref(), Some("w1:p3"));
}
