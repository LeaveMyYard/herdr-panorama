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

fn panel() -> Panel {
    Panel::new(Some("w1:p92".into()), Some("w1:t2".into()))
}

#[test]
fn the_docked_entry_is_this_tabs_and_the_selected_row_the_shown_one() {
    let mut p = panel();
    p.on_state(state(json!({"visible_tabs": ["w1:t4"]})));
    assert_eq!(p.swarm.docked.as_deref(), Some("w1:p2"));
    assert_eq!(p.view.orch.as_deref(), Some("w1:p4"));
    assert!(!p.tab_shown);
    p.on_state(state(json!({"visible_tabs": ["w1:t2"]})));
    assert_eq!(p.view.orch.as_deref(), Some("w1:p2"));
    assert!(p.tab_shown);
    // unknown visibility: shown, and the focused pane decides
    p.on_state(state(
        json!({"visible_tabs": null, "focused_pane": "w1:p3"}),
    ));
    assert!(p.tab_shown);
    assert_eq!(p.view.orch.as_deref(), Some("w1:p3"));
}

#[test]
fn a_filter_that_hides_the_shown_one_selects_nothing_until_it_shows_it_again() {
    let mut p = panel();
    p.on_state(state(json!({"visible_tabs": ["w1:t5"]}))); // Docs Site: all done
    assert_eq!(p.view.orch.as_deref(), Some("w1:p5"));
    p.on_state(state(
        json!({"visible_tabs": ["w1:t5"], "filter": "running"}),
    ));
    assert_eq!(p.view.filter, Filter::Running);
    assert_eq!(p.view.orch, None);
    assert_eq!(p.parked(), Some("w1:p5"));
    p.on_state(state(json!({"visible_tabs": ["w1:t5"], "filter": "all"})));
    assert_eq!(p.view.orch.as_deref(), Some("w1:p5"));
}

#[test]
fn the_focus_look_follows_herdrs_focus_and_the_terminals() {
    let mut p = panel();
    p.on_state(state(json!({"focused_pane": "w1:p2"})));
    assert!(!p.pane_focused);
    p.on_state(state(json!({"focused_pane": "w1:p92"})));
    assert!(p.pane_focused);
    p.on_terminal_focus(false);
    assert!(!p.pane_focused);
}

#[test]
fn a_tab_without_an_orchestrator_shows_the_narrow_layout_with_the_shown_one_opened() {
    use crate::panel::paint::Frame;
    let mut p = Panel::new(Some("w1:p99".into()), Some("w1:t9".into()));
    p.on_connected(true);
    p.on_state(state(json!({"visible_tabs": ["w1:t1"]})));
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
    // docked when its tab has one
    let mut p = panel();
    p.on_connected(true);
    p.on_state(state(json!({"visible_tabs": ["w1:t1"]})));
    assert!(p.paint(46, 45, &f).text().starts_with(" SWARM"));
}
