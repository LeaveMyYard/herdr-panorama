//! The collector over herdr snapshots shaped by hand where no transcript is needed (Python
//! `test_dock_panel.py`'s collector row): the docked agent, tab numbers, herdr down.

use panorama::collect::{Collector, Options};
use serde_json::json;

fn collector(dir: &std::path::Path) -> Collector {
    Collector::new(Options {
        projects: dir.to_path_buf(),
        events_dir: dir.join("events"),
        hooks_dir: dir.join("hooks"),
        procs: false,
        pane_pid: Box::new(|_| None),
    })
}

#[test]
fn collector_finds_the_docked_agent_and_tab_numbers() {
    let dir = tempfile::tempdir().unwrap();
    let snap = json!({
        "tabs": [{"tab_id": "w1:t1", "label": "a", "workspace_id": "w1"},
                 {"tab_id": "w2:t5", "label": "x", "workspace_id": "w2"},
                 {"tab_id": "w1:t7", "label": "b", "workspace_id": "w1"}],
        "panes": [{"pane_id": "w1:p9", "tab_id": "w1:t7", "workspace_id": "w1", "label": "◆ panorama"}],
        "agents": [{"pane_id": "w1:p1", "tab_id": "w1:t1", "workspace_id": "w1", "agent": "claude"},
                   {"pane_id": "w1:p7", "tab_id": "w1:t7", "workspace_id": "w1", "agent": "claude"}]
    });
    let mut c = collector(dir.path());
    assert_eq!(
        c.collect(Some(&snap), None, 0.0, 3600.0, 3600.0).docked,
        None
    ); // no own pane: never docked
    c.self_pane = Some("w1:p9".into());
    let sw = c.collect(Some(&snap), None, 0.0, 3600.0, 3600.0);
    assert_eq!(sw.docked.as_deref(), Some("w1:p7"));
    let got: Vec<_> = sw
        .orchestrators
        .iter()
        .map(|o| (o.pane_id.as_str(), o.tab_no))
        .collect();
    assert_eq!(got, [("w1:p1", Some(1)), ("w1:p7", Some(2))]);
    let labels: Vec<_> = sw
        .orchestrators
        .iter()
        .map(|o| o.tab_label.as_str())
        .collect();
    assert_eq!(labels, ["a", "b"]);
}

#[test]
fn herdr_down_or_an_empty_snapshot_has_no_orchestrators() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = collector(dir.path());
    let down = c.collect(None, None, 5.0, 3600.0, 60.0);
    assert!(!down.herdr_ok && !down.inbox_ok && down.orchestrators.is_empty());
    assert_eq!((down.generated_at, down.stale_after), (5.0, 60.0));
    let empty = c.collect(Some(&json!({})), Some(&[]), 5.0, 3600.0, 60.0);
    assert!(empty.herdr_ok && empty.inbox_ok && empty.orchestrators.is_empty());
}

#[test]
fn two_agents_in_one_tab_carry_their_pane_and_others_are_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let snap = json!({
        "tabs": [{"tab_id": "w1:t1", "label": "work", "workspace_id": "w1"}],
        "agents": [{"pane_id": "w1:p2", "tab_id": "w1:t1", "agent": "claude"},
                   {"pane_id": "w1:p1", "tab_id": "w1:t1", "agent": "claude", "agent_status": "blocked"},
                   {"pane_id": "w1:p3", "tab_id": "w1:t1", "agent": "codex"},
                   {"pane_id": "w1:p4", "tab_id": "w9:t9", "agent": "claude", "terminal_title_stripped": "zsh"}]
    });
    let sw = collector(dir.path()).collect(Some(&snap), None, 0.0, 3600.0, 3600.0);
    let got: Vec<_> = sw
        .orchestrators
        .iter()
        .map(|o| {
            (
                o.pane_id.as_str(),
                o.tab_label.as_str(),
                o.herdr_status.code(),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            ("w1:p1", "work · w1:p1", "blocked"),
            ("w1:p2", "work · w1:p2", "unknown"),
            ("w1:p4", "zsh", "unknown")
        ]
    );
    assert_eq!(sw.orchestrators[2].tab_no, None); // a tab herdr did not list
}
