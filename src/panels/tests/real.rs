//! The manager's herdr side (`RealOps`) against the kit's scripted herdr, with replies shaped
//! as herdr 0.9.3 answered on an isolated server (the kit's recordings).

use crate::panels::real::RealOps;
use crate::panels::{Ops, Shell};
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use serde_json::json;

fn ops(fake: &FakeHerdr) -> RealOps {
    RealOps {
        herdr: fake.client(),
        plugin_id: "leavemyyard.herdr-panorama".into(),
        root: Some("/plugins/p".into()),
        width: 46,
    }
}

fn procs(shell: i64, argv: &[&str], pid: i64) -> Reply {
    Reply::Result(
        json!({"type": "pane_process_info", "process_info": {"pane_id": "w1:p2", "shell_pid": shell,
        "foreground_processes": [{"pid": pid, "name": "x", "argv": argv}]}}),
    )
}

#[tokio::test(flavor = "current_thread")]
async fn a_panel_opens_split_right_of_its_agent_without_focus() {
    let fake = FakeHerdr::start();
    fake.on(
        "plugin.pane.open",
        Reply::Result(json!({"plugin_pane": {"pane": {"pane_id": "w1:p7", "tab_id": "w1:t2"}}})),
    );
    assert_eq!(ops(&fake).open_panel("w1:p2").await, Ok("w1:p7".into()));
    let p = &fake.params_of("plugin.pane.open")[0];
    assert_eq!(p["plugin_id"], "leavemyyard.herdr-panorama");
    assert_eq!(p["entrypoint"], "panel");
    assert_eq!(p["placement"], "split");
    assert_eq!(p["direction"], "right");
    assert_eq!(p["target_pane_id"], "w1:p2");
    assert_eq!(p["focus"], false);
    assert_eq!(p["cwd"], "/plugins/p");
    fake.on("plugin.pane.open", Reply::Result(json!({"type": "ok"})));
    assert!(
        ops(&fake).open_panel("w1:p2").await.is_err(),
        "no pane in the reply"
    );
    fake.on(
        "plugin.pane.open",
        Reply::error("plugin_not_found", "plugin not found"),
    );
    assert!(ops(&fake).open_panel("w1:p2").await.is_err());
}

#[tokio::test(flavor = "current_thread")]
async fn the_panel_is_fitted_to_its_drawable_width() {
    // layout.export / pane.layout as recorded: the panel w1:p6 second in a 120-column split
    let fake = FakeHerdr::start();
    fake.on("layout.export", Reply::Result(json!({"type": "layout_export", "layout": {"tab_id": "w1:t2",
        "root": {"type": "split", "direction": "right", "ratio": 0.5,
                 "first": {"type": "pane", "pane_id": "w1:p2"}, "second": {"type": "pane", "pane_id": "w1:p6"}}}})));
    fake.on(
        "pane.layout",
        Reply::Result(
            json!({"type": "pane_layout", "layout": {"tab_id": "w1:t2", "panes": [
        {"pane_id": "w1:p2", "rect": {"x": 0, "y": 0, "width": 60, "height": 40}},
        {"pane_id": "w1:p6", "rect": {"x": 60, "y": 0, "width": 60, "height": 40}}]}}),
        ),
    );
    fake.on(
        "layout.set_split_ratio",
        Reply::Result(json!({"type": "layout_split_ratio_set", "layout": {}})),
    );
    ops(&fake).fit("w1:p6", "w1:t2").await;
    let set = fake.params_of("layout.set_split_ratio");
    assert_eq!(set.len(), 1);
    assert_eq!(set[0]["tab_id"], "w1:t2");
    assert_eq!(set[0]["path"], json!([]));
    assert!((set[0]["ratio"].as_f64().unwrap() - (1.0 - 48.0 / 120.0)).abs() < 1e-9);
}

#[tokio::test(flavor = "current_thread")]
async fn a_pane_runs_our_panel_only_by_its_argv() {
    let fake = FakeHerdr::start();
    fake.on(
        "pane.process_info",
        procs(
            10,
            &["/p/.venv/bin/python", "/p/.venv/bin/panorama", "panel"],
            10,
        ),
    );
    assert_eq!(ops(&fake).runs_panel("w1:p2").await, Some(true));
    assert_eq!(ops(&fake).pane_shell("w1:p2").await, Shell::Busy);
    fake.on("pane.process_info", procs(10, &["/usr/bin/zsh"], 10));
    assert_eq!(ops(&fake).runs_panel("w1:p2").await, Some(false));
    assert_eq!(ops(&fake).pane_shell("w1:p2").await, Shell::Plain(10));
    fake.on(
        "pane.process_info",
        Reply::error("pane_not_found", "pane not found"),
    );
    assert_eq!(ops(&fake).runs_panel("w1:p2").await, None);
    assert_eq!(ops(&fake).pane_shell("w1:p2").await, Shell::Unknown);
}

#[tokio::test(flavor = "current_thread")]
async fn closing_swapping_and_focusing_name_the_panes() {
    let fake = FakeHerdr::start();
    for m in ["pane.close", "plugin.pane.close", "tab.focus", "pane.focus"] {
        fake.on(m, Reply::Result(json!({"type": "ok"})));
    }
    fake.on(
        "pane.swap",
        Reply::Result(json!({"type": "pane_swap", "swap": {"changed": true}})),
    );
    let o = ops(&fake);
    o.close_pane("w1:p3").await.unwrap();
    o.close_plugin_pane("w1:p4").await.unwrap();
    o.swap("w1:p2", "w1:p7").await.unwrap();
    assert_eq!(fake.params_of("pane.close")[0]["pane_id"], "w1:p3");
    assert_eq!(fake.params_of("plugin.pane.close")[0]["pane_id"], "w1:p4");
    let swap = &fake.params_of("pane.swap")[0];
    assert_eq!(
        (
            swap["source_pane_id"].as_str(),
            swap["target_pane_id"].as_str()
        ),
        (Some("w1:p2"), Some("w1:p7"))
    );
    fake.on(
        "pane.close",
        Reply::error("pane_not_found", "pane not found"),
    );
    assert!(o.close_pane("w1:p3").await.is_err());
    // herdr refuses a swap across tabs with an answer, not an error: still a failed swap
    fake.on(
        "pane.swap",
        Reply::Result(
            json!({"type": "pane_swap", "swap": {"changed": false, "reason": "cross_tab"}}),
        ),
    );
    assert_eq!(
        o.swap("w1:p2", "w1:p9").await,
        Err("not swapped: cross_tab".into())
    );
}

/// herdr 0.9.3's `session.snapshot` reply as recorded on an isolated server (the kit's
/// `fixtures/herdr/api.jsonl`, seq 0): three tabs, Claude in `w1:p2` with the focus.
fn recorded() -> FakeHerdr {
    let fake = FakeHerdr::start();
    fake.replay("api.jsonl", 0);
    fake
}

#[tokio::test(flavor = "current_thread")]
async fn herdrs_snapshot_reads_as_the_manager_sees_it() {
    let snap = ops(&recorded())
        .snapshot()
        .await
        .expect("the recorded snapshot");
    assert_eq!(snap.focused_tab.as_deref(), Some("w1:t2"));
    assert_eq!(snap.focused_pane.as_deref(), Some("w1:p2"));
    let p = |id: &str| snap.pane(id).unwrap().clone();
    assert_eq!(p("w1:p1").tab_id.as_deref(), Some("w1:t1"));
    assert_eq!(p("w1:p1").workspace_id.as_deref(), Some("w1"));
    assert_eq!(p("w1:p1").agent, None);
    assert_eq!(p("w1:p2").agent.as_deref(), Some("claude"));
    assert!(p("w1:p2").focused && !p("w1:p1").focused && !p("w1:p3").focused);
    assert_eq!(
        p("w1:p1").terminal_id.as_deref(),
        Some("term_65e0a20f144c11")
    );
    assert_eq!(
        p("w1:p3").terminal_id.as_deref(),
        Some("term_65e0a20fc0d3c3")
    );
    assert_eq!(p("w1:p1").label, None);
    // each tab's remembered focus (layouts[].focused_pane_id)
    assert!(snap.tab_remembers("w1:p1") && snap.tab_remembers("w1:p2"));
    assert!(!snap.tab_remembers("w1:p9"));
}

#[tokio::test(flavor = "current_thread")]
async fn a_new_terminal_in_herdrs_snapshot_starts_the_restore_check() {
    // end to end through RealOps: a recorded panel whose terminal herdr now reports as
    // another one is judged (here left alone: herdr shows it without the panel label)
    use crate::panels::{FocusLog, Manager, Places};
    use std::sync::Arc;
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("panels.json");
    std::fs::write(
        &state,
        r#"{"closed": [], "panes": {"w1:p3": {"tab": "w1:t3", "terminal": "term_before"}}}"#,
    )
    .unwrap();
    let places = Places {
        state_path: Some(state),
        marker_dir: None,
        home_lock: dir.path().join("home.lock"),
    };
    let fake = recorded();
    let log = Arc::new(FocusLog::default());
    log.set_live(true);
    let mut m = Manager::new(
        ops(&fake),
        Some(log),
        places,
        Arc::new(std::time::Instant::now),
    );
    let snap = ops(&fake).snapshot().await.unwrap();
    m.reconcile(
        snap,
        &super::world::orch(&[]),
        None,
        &Default::default(),
        false,
        false,
    )
    .await
    .unwrap();
    let notes = m.take_notes();
    assert!(
        notes
            .iter()
            .any(|n| n == "w1:p3 was a panel before herdr restarted; left alone (relabelled)"),
        "{notes:?}"
    );
    assert!(m.record().is_empty());
}
