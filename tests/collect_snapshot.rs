//! The collector over herdr snapshots shaped by hand where no transcript is needed (Python
//! `test_dock_panel.py`'s collector row): the docked agent, tab numbers, herdr down.

use panorama::collect::{Collector, Options};
use serde_json::json;

fn collector(dir: &std::path::Path) -> Collector {
    Collector::new(Options {
        projects: dir.to_path_buf(),
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
        .roots
        .iter()
        .map(|o| (o.pane_id.as_str(), o.tab_no))
        .collect();
    assert_eq!(got, [("w1:p1", Some(1)), ("w1:p7", Some(2))]);
    let labels: Vec<_> = sw.roots.iter().map(|o| o.tab_label.as_str()).collect();
    assert_eq!(labels, ["a", "b"]);
}

#[test]
fn herdr_down_or_an_empty_snapshot_has_no_orchestrators() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = collector(dir.path());
    let down = c.collect(None, None, 5.0, 3600.0, 60.0);
    assert!(!down.herdr_ok && !down.inbox_ok && down.roots.is_empty());
    assert_eq!((down.generated_at, down.stale_after), (5.0, 60.0));
    let empty = c.collect(Some(&json!({})), Some(&[]), 5.0, 3600.0, 60.0);
    assert!(empty.herdr_ok && empty.inbox_ok && empty.roots.is_empty());
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
        .roots
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
            ("w1:p3", "work · w1:p3", "unknown"),
            ("w1:p4", "zsh", "unknown")
        ]
    );
    assert_eq!(sw.roots[3].tab_no, None); // a tab herdr did not list
}

#[test]
fn codex_without_history_keeps_exact_identity_and_never_reads_claude_history() {
    let dir = tempfile::tempdir().unwrap();
    // A Claude transcript with the same id must not enrich the Codex pane.
    let project = dir.path().join("-synthetic");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join("same.jsonl"),
        "{\"type\":\"summary\",\"summary\":\"wrong harness\"}\n",
    )
    .unwrap();
    let snap = json!({
        "tabs": [{"tab_id": "t", "label": "shared", "workspace_id": "w"}],
        "panes": [{"pane_id": "panel", "tab_id": "t"}],
        "agents": [
            {"pane_id": "c1", "tab_id": "t", "agent": "claude"},
            {"pane_id": "x1", "tab_id": "t", "agent": "codex", "agent_status": "busy",
             "workspace_id": "w", "focused": true, "cwd": "/synthetic",
             "agent_session": {"kind": "id", "value": "same", "source":"herdr:codex", "agent":"codex"}},
            {"pane_id": "x2", "tab_id": "t", "agent": "codex", "cwd": "/synthetic"},
            {"pane_id": "x3", "tab_id": "t", "agent": "codex",
             "agent_session": {"kind": "path", "value": "same"}},
            {"pane_id": "other", "tab_id": "t", "agent": "opencode"}
        ]
    });
    let mut c = collector(dir.path());
    c.self_pane = Some("panel".into());
    let sw = c.collect(Some(&snap), None, 1.0, 3600.0, 60.0);
    assert_eq!(sw.roots.len(), 4);
    let x = &sw.roots[1];
    assert_eq!(
        (&*x.harness, x.session_id.as_deref()),
        ("codex", Some("same"))
    );
    assert_eq!((&*x.tab_label, x.tab_no), ("shared · x1", Some(1)));
    assert_eq!(x.herdr_status.code(), "busy");
    assert!(x.focused);
    assert_eq!(x.cwd.as_deref(), Some("/synthetic"));
    assert_eq!(x.workspace_id.as_deref(), Some("w"));
    for x in &sw.roots[1..] {
        assert!(x.transcript.is_none() && x.agents.is_empty() && x.last_activity.is_none());
    }
    assert!(sw.roots[2].session_id.is_none());
    assert!(sw.roots[3].session_id.is_none());
    // A Codex-only tab is dockable too.
    let mut only = snap;
    only["agents"].as_array_mut().unwrap().remove(0);
    assert_eq!(
        c.collect(Some(&only), None, 1.0, 3600.0, 60.0)
            .docked
            .as_deref(),
        Some("x1")
    );
}
