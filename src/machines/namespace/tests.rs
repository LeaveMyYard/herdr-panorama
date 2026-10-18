use super::*;
use panorama_core::model::Orchestrator;

fn orch(pane: &str, tab: Option<&str>, label: &str) -> Orchestrator {
    serde_json::from_value(serde_json::json!({
        "pane_id": pane, "tab_id": tab, "tab_label": label, "session_id": "s",
    }))
    .unwrap()
}

fn devbox() -> Machine {
    Machine::new("abc123", "devbox", "devbox")
}

#[test]
fn namespacing_keeps_machines_apart() {
    let mut o = orch("w1:p1", Some("w1:t1"), "a");
    o.inbox = vec![3];
    o.inbox_texts = vec!["q".into()];
    let mut sw = Swarm {
        orchestrators: vec![o],
        ..Swarm::default()
    };
    sw.docked = Some("w1:p1".into());
    let sw = namespace(sw, &devbox());
    let o = &sw.orchestrators[0];
    assert_eq!(o.pane_id, "abc123:w1:p1");
    assert_eq!(o.tab_id.as_deref(), Some("abc123:w1:t1"));
    assert_eq!(o.machine.as_deref(), Some("devbox"));
    assert_eq!(o.machine_id.as_deref(), Some("abc123"));
    assert!(o.inbox.is_empty() && o.inbox_texts.is_empty());
    assert_eq!(sw.docked, None); // never docked into a remote server
}

#[test]
fn a_missing_or_empty_tab_id_stays_as_it_is() {
    let sw = Swarm {
        orchestrators: vec![orch("w1:p1", None, "a"), orch("w1:p2", Some(""), "b")],
        ..Swarm::default()
    };
    let sw = namespace(sw, &devbox());
    assert_eq!(sw.orchestrators[0].tab_id, None);
    assert_eq!(sw.orchestrators[1].tab_id.as_deref(), Some(""));
}

#[test]
fn two_machines_with_one_label_stay_apart_and_colons_are_fine() {
    let mut ids = Vec::new();
    for mid in ["aaa", "bbb"] {
        let part = Swarm {
            orchestrators: vec![orch("w1:p1", Some("w1:t1"), &format!("work on {mid}"))],
            ..Swarm::default()
        };
        let sw = namespace(part, &Machine::new(mid, "lab:1", "host"));
        ids.push(sw.orchestrators[0].pane_id.clone());
    }
    assert_eq!(ids, ["aaa:w1:p1", "bbb:w1:p1"]);
    assert_eq!(local_id(&ids[0], "aaa"), "w1:p1");
    assert_eq!(local_id("w1:p1", "aaa"), "w1:p1");
    assert_eq!(local_id("lab:1:w1:p1", "lab:1"), "w1:p1");
}
