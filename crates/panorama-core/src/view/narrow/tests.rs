//! Narrow-layout cases the goldens do not reach (70 × 45): the uniform tree's cards (plan
//! `uniform-agent-tree.md` §4), the fold of finished subagents, machine groups, scrolling.

use super::*;
use serde_json::{Value, json};

fn orch(pane: &str, extra: Value) -> Value {
    let mut o = json!({"pane_id": pane, "tab_id": null, "tab_label": pane, "session_id": null});
    o.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    o
}

fn rows(swarm: &Swarm, view: &View) -> (Screen, Vec<String>) {
    let s = page(swarm, view, 70, 45, None);
    let text = s.text().lines().map(str::to_owned).collect();
    (s, text)
}

/// A root with a live subagent `a` (its children `a1` live, `a2` done), and two finished
/// ones; the done window dropped two more.
fn tree() -> Swarm {
    let ag = |id: &str, st: &str, parent: Option<&str>, n: i64| {
        json!({"id": id, "title": format!("task {id}"), "status": st, "parent": parent,
               "started": n, "seq": n, "model": "opus"})
    };
    serde_json::from_value(json!({"orchestrators": [
        orch("p1", json!({"hidden_finished": 2, "threads": [
            {"id": crate::model::TREE_THREAD, "name": "agents", "source": "session", "agents": [
                ag("a", "run", None, 1), ag("a1", "run", Some("a"), 2),
                ag("a2", "done", Some("a"), 3), ag("d1", "done", None, 4),
                ag("d2", "done", None, 5),
            ]},
        ]})),
        orch("p2", json!({"machine": "box", "machine_id": "m1"})),
    ]}))
    .unwrap()
}

#[test]
fn a_card_per_direct_subagent_its_descendants_as_pods_and_the_selected_ones_details() {
    let view = View {
        orch: Some("p1".into()),
        agent: Some("a1".into()),
        focus: 2,
        ..View::default()
    };
    let (s, r) = rows(&tree(), &view);
    assert!(r[2].ends_with("  Claude Code · p1  working"), "{r:#?}");
    assert!(
        r[5].starts_with(" ├─┨ ") && r[5].contains(" task a "),
        "{r:#?}"
    );
    // its descendants' pods, the selected one bracketed; then its model and age
    assert!(
        r[6].starts_with(" │ ┃  [ ") && r[6].contains(" ] ✓ "),
        "{r:#?}"
    );
    assert!(r[6].ends_with("opus · 0m │"), "{r:#?}");
    // the selected agent's details, in the card that holds it
    assert!(r[8].contains("task a1 agent a1 · opus"), "{r:#?}");
    assert!(r.iter().any(|l| l.contains("g open pane")), "{r:#?}");
    let fin = r.iter().position(|l| l.contains("finished")).unwrap();
    assert_eq!(r[fin], " ╰─  ✓ +2 older · 2 finished · task d1, task d2");
    assert!(s.targets.iter().any(|h| h.target
        == crate::view::target::Target::Fin {
            pane: "p1".into(),
            group: None
        }));
    let box_at = r.iter().position(|l| l.starts_with("─ box ─")).unwrap();
    assert_eq!(r[box_at + 1], "  ✓  Claude Code · p2  all done");
    assert_eq!(r[box_at + 3], "  +   New agent  n");
}

#[test]
fn the_fold_selected_and_unfolded() {
    let mut view = View {
        orch: Some("p1".into()),
        agent: Some(format!("{}p1", crate::view::state::FIN_CARD)),
        focus: 1,
        ..View::default()
    };
    let (s, r) = rows(&tree(), &view);
    let fin = r.iter().position(|l| l.contains("finished")).unwrap();
    assert!(r[fin].starts_with(" ╰─▸ ✓"), "{r:#?}");
    assert_eq!(s.cell(3, fin as i32).fg, HI); // selected and focused
    view.fin_open.insert("p1".into());
    let (_, r) = rows(&tree(), &view);
    // unfolded: the finished subagents get their cards after the fold
    let fin = r.iter().position(|l| l.contains("finished")).unwrap();
    assert!(
        r[fin + 2].contains("✓ task d1") && r[fin + 6].contains("✓ task d2"),
        "{r:#?}"
    );
}

#[test]
fn a_long_list_scrolls_to_the_selection() {
    let orchs: Vec<Value> = (0..30).map(|i| orch(&format!("q{i}"), json!({}))).collect();
    let swarm: Swarm = serde_json::from_value(json!({ "orchestrators": orchs })).unwrap();
    let (s, r) = rows(
        &swarm,
        &View {
            orch: Some("q29".into()),
            ..View::default()
        },
    );
    assert_eq!(s.columns[0].offset, 19);
    assert_eq!(r[39], "  ✓  Claude Code · q28  all done");
    assert_eq!(r[41], "▌ ✓  Claude Code · q29  all done");
    // the narrow legend at the bottom
    assert_eq!(
        r[42],
        " ✓ done ✓? done? ⠋ run » rev ○ queue ? ask ! block ✗ fail ~ lost"
    );
}

#[test]
fn a_wide_pane_keeps_the_narrow_chrome() {
    let swarm: Swarm =
        serde_json::from_value(json!({"orchestrators": [orch("p1", json!({}))]})).unwrap();
    let s = page(&swarm, &View::default(), 130, 45, None);
    let text = s.text();
    let keys = text.lines().last().unwrap();
    assert!(keys.starts_with(" ↑↓ move   → open   g pane"), "{keys:?}");
}
