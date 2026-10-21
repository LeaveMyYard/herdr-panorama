//! Remote machines in the views (Python tests/test_machines.py): a remote group has one
//! divider with its machine's label, never clickable; the same local ids on two machines are
//! two identities; two machines with one label stay apart.

use panorama::machines::{Machine, local_id, namespace};
use panorama_core::model::Swarm;
use panorama_core::view::nav::{find_orch, nav};
use panorama_core::view::{View, render_dock, render_graph};
use serde_json::Value;

const NO_SESSIONS: &str = "No Claude Code sessions in herdr";

/// A captured wire swarm (`tests/fixtures/inputs/swarm-<name>.json`).
fn wire(name: &str) -> Swarm {
    let path = format!(
        "{}/tests/fixtures/inputs/swarm-{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    serde_json::from_value(v["swarm"].clone()).unwrap()
}

fn rows(s: &panorama_core::view::Screen) -> Vec<String> {
    s.text().split('\n').map(str::to_owned).collect()
}

#[test]
fn remote_groups_get_a_divider_and_it_is_not_clickable() {
    // the captured mixed swarm (Python's `mixed_swarm`): the demo plus devbox's "Remote work"
    for (w, dock) in [(160, false), (70, false), (46, true)] {
        let mut sw = wire("remote");
        let v = View {
            orch: Some("w1:p1".into()),
            ..View::default()
        };
        let s = if dock {
            sw.docked = Some("w1:p1".into());
            render_dock(&sw, &v, w, 60, None, NO_SESSIONS)
        } else {
            render_graph(&sw, &v, w, 60, None, NO_SESSIONS)
        };
        let rows = rows(&s);
        let y = rows
            .iter()
            .position(|r| r.contains(" devbox "))
            .unwrap_or_else(|| panic!("{w}: no divider"));
        assert!(
            rows[y].trim().starts_with("─ devbox ─"),
            "{w}: {:?}",
            rows[y]
        );
        let span = if w >= 120 { 0..32 } else { 0..w };
        assert!(
            span.clone().all(|x| s.target_at(x, y as i32).is_none()),
            "{w}: the divider is clickable"
        );
        assert_eq!(
            rows.iter().filter(|r| r.contains(" devbox ")).count(),
            1,
            "{w}: one divider, nothing on cards"
        );
        assert!(
            rows[y + 1..].iter().any(|r| r.contains("Remote work")),
            "{w}: the group follows its divider"
        );
    }
}

#[test]
fn two_machines_with_the_same_local_ids_are_two_identities() {
    let sw = wire("remote");
    assert_eq!(find_orch(&sw, Some("w1:p1")).unwrap().machine, None);
    assert_eq!(
        find_orch(&sw, Some("abc123:w1:p1"))
            .unwrap()
            .machine
            .as_deref(),
        Some("devbox")
    );
    let v = View {
        orch: Some("w1:p1".into()),
        ..View::default()
    };
    assert_eq!(
        nav(&sw, &v)[0].iter().filter(|id| *id == "w1:p1").count(),
        1
    );
}

#[test]
fn two_machines_with_one_label_stay_apart_and_colons_are_fine() {
    let mut sw = wire("demo");
    let local = sw.orchestrators[0].clone();
    for mid in ["aaa", "bbb"] {
        let mut o = local.clone();
        o.pane_id = "w1:p1".into();
        o.tab_id = Some("w1:t1".into());
        o.tab_label = format!("work on {mid}");
        let part = Swarm {
            orchestrators: vec![o],
            ..Swarm::default()
        };
        sw.orchestrators
            .extend(namespace(part, &Machine::new(mid, "lab:1", "host")).orchestrators);
    }
    let ids: Vec<&str> = sw.orchestrators[sw.orchestrators.len() - 2..]
        .iter()
        .map(|o| o.pane_id.as_str())
        .collect();
    assert_eq!(ids, ["aaa:w1:p1", "bbb:w1:p1"]);
    assert_eq!(local_id(ids[0], "aaa"), "w1:p1");
    let v = View {
        orch: Some("w1:p1".into()),
        ..View::default()
    };
    let rows = rows(&render_graph(&sw, &v, 160, 70, None, NO_SESSIONS));
    assert_eq!(
        rows.iter().filter(|r| r.contains(" lab:1 ")).count(),
        2,
        "one group (divider) per machine id"
    );
}
