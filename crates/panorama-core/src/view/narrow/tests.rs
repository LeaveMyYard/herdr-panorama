//! Narrow-layout cases the goldens do not reach, with rows read once from Python's
//! `render.render` for the same input (70 × 45).

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

#[test]
fn the_done_card_and_a_machine_group() {
    let swarm: Swarm = serde_json::from_value(json!({"orchestrators": [
        orch("p1", json!({"threads": [
            {"id": "t1", "name": "live", "source": "turn", "agents": [{"id": "a", "status": "run"}]},
            {"id": "d1", "name": "old one", "source": "turn", "status": "done"},
            {"id": "d2", "name": "old two", "source": "turn", "status": "done"},
        ]})),
        orch("p2", json!({"machine": "box", "machine_id": "m1"})),
    ]}))
    .unwrap();
    // interim (plan `uniform-agent-tree.md` U4), until U5 replaces narrow: the done card no
    // longer unfolds (no thread is selected)
    let view = View {
        orch: Some("p1".into()),
        focus: 1,
        ..View::default()
    };
    let (_, r) = rows(&swarm, &view);
    assert!(
        r.iter()
            .any(|l| l == " ╰─  ✓ 2 complete · old one, old two"),
        "{r:#?}"
    );
    let box_at = r.iter().position(|l| l.starts_with("─ box ─")).unwrap();
    assert_eq!(r[box_at + 1], "  ✓  Claude Code · p2  all done");
    assert_eq!(r[box_at + 3], "  +   New agent  n");
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
    assert_eq!(r[41], "▌ ✓  Claude Code · q29  all done  agents");
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
