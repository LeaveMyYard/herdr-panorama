//! The selection rules both views share, each pinned on its own (the panel's and the app's
//! own tests drive them through their models).

use super::*;
use panorama_core::model::Agent;
use serde_json::Value;

fn demo() -> Swarm {
    let path = format!(
        "{}/tests/fixtures/inputs/swarm-demo.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    serde_json::from_value(crate::v1_wire::swarm(v["swarm"].clone())).unwrap()
}

/// The demo with its first root and its first depth-1 agent selected.
fn selected() -> (Swarm, View) {
    let sw = demo();
    let mut v = View::default();
    initial(&sw, &mut v, Some("w1:p1"));
    (sw, v)
}

/// The selected agent of `sw` under `v`, to change.
fn agent<'a>(sw: &'a mut Swarm, v: &View) -> &'a mut Agent {
    let o = sw
        .roots
        .iter_mut()
        .find(|o| Some(&o.pane_id) == v.orch.as_ref())
        .unwrap();
    o.agents
        .iter_mut()
        .find(|a| Some(&a.id) == v.agent.as_ref())
        .unwrap()
}

#[test]
fn every_part_of_the_selected_card_changes_it() {
    let (sw, v) = selected();
    let card = |sw: &Swarm, v: &View, dock| selected_card(sw, v, dock);
    let base = card(&sw, &v, false);
    assert_eq!(base, card(&sw, &v, false));
    assert_ne!(base, card(&sw, &v, true), "dock");
    let mut v2 = v.clone();
    v2.focus = 2;
    assert_ne!(base, card(&sw, &v2, false), "focus");
    let changed = |f: &dyn Fn(&mut Swarm)| {
        let mut sw2 = sw.clone();
        f(&mut sw2);
        card(&sw2, &v, false)
    };
    assert_ne!(
        base,
        changed(&|s| s.roots[0].herdr_status = HerdrStatus::Idle),
        "orch status"
    );
    assert_ne!(base, changed(&|s| s.roots[0].inbox.push(99)), "orch inbox");
    assert_ne!(base, changed(&|s| s.inbox_ok = !s.inbox_ok), "inbox_ok");
    assert_ne!(
        base,
        changed(&|s| agent(s, &v).status = Status::Lost),
        "agent status"
    );
    assert_ne!(
        base,
        changed(&|s| agent(s, &v).question_ids.push(99)),
        "question ids"
    );
    assert_ne!(
        base,
        changed(&|s| agent(s, &v).transcript = Some("/x".into())),
        "transcript"
    );
    let mut v3 = v.clone();
    v3.agent = None;
    assert_ne!(base, card(&sw, &v3, false), "no agent");
    let mut v4 = v.clone();
    v4.agent = Some("a2f81c0".into());
    assert_ne!(base, card(&sw, &v4, false), "agent");
}

#[test]
fn the_rows_end_with_new_agent_even_with_no_orchestrator() {
    let (sw, v) = selected();
    let ids = shown_ids(&sw, &v);
    assert_eq!(ids.last().map(String::as_str), Some(NEW_ROW));
    assert_eq!(ids.len(), sw.roots.len() + 1);
    assert_eq!(shown_ids(&Swarm::default(), &View::default()), [NEW_ROW]);
}

#[test]
fn the_first_selection_is_the_focus_pane_else_the_first_row_in_column_0() {
    let sw = demo();
    let mut v = View {
        focus: 3,
        ..View::default()
    };
    initial(&sw, &mut v, Some("w1:p2"));
    assert_eq!((v.focus, v.orch.as_deref()), (0, Some("w1:p2")));
    assert_eq!(v.agent.as_deref(), Some("j1a2b3c"));
    let mut v = View {
        focus: 2,
        ..View::default()
    };
    initial(&sw, &mut v, Some("w9:p9"));
    assert_eq!((v.focus, v.orch.as_deref()), (0, Some("w1:p1")));
    let mut v = View::default();
    initial(&Swarm::default(), &mut v, None);
    assert_eq!(v.orch.as_deref(), Some(NEW_ROW));
}

#[test]
fn resolve_parks_a_vanished_orchestrator_and_clears_what_vanished_below() {
    let (sw, mut v) = selected();
    assert_eq!(resolve(&sw, &mut v), None); // all still there
    assert!(v.orch.is_some() && v.agent.is_some());
    let mut gone = sw.clone();
    let a = v.agent.clone().unwrap();
    gone.roots[0].agents.retain(|x| x.id != a);
    assert_eq!(resolve(&gone, &mut v), None);
    assert!(v.orch.is_some() && v.agent.is_none()); // the agent vanished
    let mut gone = sw.clone();
    gone.roots.remove(0);
    let (_, mut v) = selected();
    assert_eq!(resolve(&gone, &mut v).as_deref(), Some("w1:p1"));
    assert!(v.orch.is_none() && v.agent.is_none());
}

#[test]
fn the_arming_window_is_pythons() {
    assert_eq!(ARM, Duration::from_millis(400));
}
