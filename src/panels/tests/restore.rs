//! After herdr stops unexpectedly (Python `tests/test_panels.py`, the operator's report of
//! 2026-10-17): restored panels are closed only when provably our unused panels.

use super::world::*;
use crate::panels::{
    Focus, FocusLog, Manager, PANEL_LABEL, RESTORE_PASSES, RESTORED_SECONDS, Shell,
};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;

/// A core that made t2's panel and saw it running (so it is recorded); the panel.
async fn recorded_panel(w: &World, dir: &Path) -> (Manager<World>, String) {
    let mut m = manager(w, dir);
    let sw = orch(&["w1:p2"]);
    pass(&mut m, w, &sw, &[], true).await;
    pass(&mut m, w, &sw, &[], false).await;
    let panel = w.order("w1:t2")[0].clone();
    (m, panel)
}

/// The core after herdr's restart: a new manager and `passes` passes (no start batch).
async fn restarted(w: &World, dir: &Path, passes: u32, sw: &[&str]) -> Manager<World> {
    let mut again = manager(w, dir);
    again.started = true;
    for _ in 0..passes {
        pass(&mut again, w, &orch(sw), &[], false).await;
    }
    again
}

const BOTH: &[&str] = &["w1:p2", "w1:p3"];

fn recorded(m: &Manager<World>, pane: &str) -> bool {
    m.record().iter().any(|(p, _, _)| p == pane)
}

#[tokio::test]
async fn a_panel_herdr_restored_as_a_plain_shell_is_closed_and_made_again_in_its_place() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let sw = orch(BOTH);
    let mut m = manager(&w, dir.path());
    pass(&mut m, &w, &sw, &[], true).await;
    pass(&mut m, &w, &sw, &[], false).await; // herdr's snapshot shows the new panels: recorded
    let old: Vec<String> = ["w1:t2", "w1:t3"]
        .iter()
        .map(|t| w.order(t)[0].clone())
        .collect();
    w.s().restart_herdr();
    let mut again = manager(&w, dir.path());
    pass(&mut again, &w, &sw, &[], true).await;
    for (i, (tab, ag)) in [("w1:t2", "w1:p2"), ("w1:t3", "w1:p3")]
        .into_iter()
        .enumerate()
    {
        assert!(w.calls().contains(&format!("close_pane {}", old[i])));
        let order = w.order(tab);
        assert_eq!(order.len(), 2);
        assert_eq!(order[1], ag);
        assert!(w.s().argv.contains_key(&order[0]));
        assert_ne!(order[0], old[i]);
    }
    assert!(
        again.closed().is_empty(),
        "closing a restored panel is not the operator closing it"
    );
}

#[tokio::test]
async fn a_restored_pane_is_closed_only_when_provably_our_unused_panel() {
    for case in [
        "never-recorded",
        "same-terminal",
        "no-terminal-recorded",
        "pane-without-terminal",
        "relabelled",
        "agent",
        "focused",
        "focused-flag",
        "tab-focus",
        "in-use",
        "background-job",
        "children-unknown",
        "old-shell",
        "age-unknown",
        "quit-cleanly",
    ] {
        let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
        let (mut m, panel) = recorded_panel(&w, dir.path()).await;
        // the operator's own shell, labelled like a panel: never recorded
        w.s().tab("w1:t4").insert(
            0,
            P {
                label: Some(PANEL_LABEL.into()),
                terminal: Some("term_x".into()),
                ..pane("w1:p60")
            },
        );
        let file = dir.path().join("panels.json");
        let edit = |f: &dyn Fn(&mut Value)| {
            let mut v: Value =
                serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
            f(&mut v);
            std::fs::write(&file, v.to_string()).unwrap();
        };
        if case == "never-recorded" {
            edit(&|v| v["panes"] = serde_json::json!({})); // e.g. a panel from before this fix
        }
        if case == "quit-cleanly" {
            m.remove_all(Some(w.snap()), &set(&[])).await;
            let p = P {
                label: Some(PANEL_LABEL.into()),
                terminal: Some("term_y".into()),
                ..pane(&panel)
            };
            w.s().tab("w1:t2").insert(0, p);
        }
        w.s().restart_herdr();
        {
            let mut s = w.s();
            match case {
                "same-terminal" => s.pane(&panel).terminal = Some(m.record()[0].2.clone()),
                "pane-without-terminal" => s.pane(&panel).terminal = None,
                "relabelled" => s.pane(&panel).label = Some("notes".into()),
                "agent" => s.pane(&panel).agent = Some("claude".into()),
                "focused" => s.focus = ("w1:t2".into(), panel.clone()),
                "focused-flag" => s.pane(&panel).focused = true,
                "tab-focus" => _ = s.tab_focus.insert("w1:t2".into(), panel.clone()),
                "in-use" => {
                    _ = s
                        .shell
                        .insert(panel.clone(), (Some(false), Some(3.0), Some(false)))
                }
                "background-job" => {
                    _ = s
                        .shell
                        .insert(panel.clone(), (Some(true), Some(3.0), Some(true)))
                }
                "children-unknown" => {
                    _ = s.shell.insert(panel.clone(), (Some(true), Some(3.0), None))
                }
                "old-shell" => {
                    _ = s.shell.insert(
                        panel.clone(),
                        (Some(true), Some(RESTORED_SECONDS + 1.0), Some(false)),
                    )
                }
                "age-unknown" => {
                    _ = s
                        .shell
                        .insert(panel.clone(), (Some(true), None, Some(false)))
                }
                _ => {}
            }
        }
        if case == "no-terminal-recorded" {
            edit(&|v| {
                _ = v["panes"][&panel]
                    .as_object_mut()
                    .unwrap()
                    .remove("terminal")
            });
        }
        let mut again = restarted(&w, dir.path(), 1, BOTH).await;
        assert!(w.closes().is_empty(), "{case}");
        assert!(
            w.order("w1:t2").contains(&panel) && w.order("w1:t4").contains(&"w1:p60".to_string()),
            "{case}"
        );
        {
            let mut s = w.s();
            let p = s.pane(&panel);
            p.agent = None;
            p.focused = false;
            p.label = Some(PANEL_LABEL.into());
            s.shell.remove(&panel);
            s.focus = ("w1:t1".into(), "w1:p1".into());
            s.tab_focus.clear();
        }
        pass(&mut again, &w, &orch(BOTH), &[], false).await; // idle now, still within the passes
        assert!(w.closes().is_empty(), "{case}: each verdict is final");
    }
}

#[tokio::test]
async fn a_pane_focused_after_the_restore_is_never_closed() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let (_, panel) = recorded_panel(&w, dir.path()).await;
    w.s().restart_herdr();
    w.s().shell.insert(panel.clone(), (None, None, None));
    let mut again = restarted(&w, dir.path(), 1, BOTH).await;
    w.s().record(Focus::Pane(panel.clone()));
    w.s().shell.remove(&panel);
    pass(&mut again, &w, &orch(BOTH), &[], false).await;
    assert!(w.closes().is_empty() && !recorded(&again, &panel));
}

#[tokio::test]
async fn a_restored_panel_is_judged_only_on_the_first_passes() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let (_, panel) = recorded_panel(&w, dir.path()).await;
    w.s().restart_herdr();
    w.s().shell.insert(panel.clone(), (None, None, None));
    let mut again = restarted(&w, dir.path(), RESTORE_PASSES, BOTH).await;
    assert!(recorded(&again, &panel));
    w.s().shell.remove(&panel); // idle and describable from now on, but too late
    pass(&mut again, &w, &orch(BOTH), &[], false).await;
    pass(&mut again, &w, &orch(BOTH), &[], false).await;
    assert!(
        w.closes().is_empty() && !recorded(&again, &panel) && w.order("w1:t2").contains(&panel)
    );
}

#[tokio::test]
async fn a_restored_panel_is_judged_on_the_last_of_its_passes() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let (_, panel) = recorded_panel(&w, dir.path()).await;
    w.s().restart_herdr();
    w.s().shell.insert(panel.clone(), (None, None, None));
    let mut again = restarted(&w, dir.path(), RESTORE_PASSES - 1, BOTH).await;
    w.s().shell.remove(&panel);
    pass(&mut again, &w, &orch(BOTH), &[], false).await;
    assert!(w.calls().contains(&format!("close_pane {panel}")));
}

#[tokio::test]
async fn a_shell_seen_busy_at_either_look_is_not_closed() {
    for (first, last) in [(true, false), (false, true)] {
        let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
        let (_, panel) = recorded_panel(&w, dir.path()).await;
        w.s().restart_herdr();
        let mut looks = vec![first, last, true, true].into_iter();
        let p = panel.clone();
        w.s().looks = Some(Box::new(move |_, pid| {
            if pid != p {
                return Shell::Plain(1);
            }
            if looks.next().unwrap() {
                Shell::Plain(7000)
            } else {
                Shell::Busy
            }
        }));
        w.s().ages = Some((Some(3.0), Some(false)));
        let mut again = restarted(&w, dir.path(), 1, &["w1:p2"]).await;
        pass(&mut again, &w, &orch(&["w1:p2"]), &[], false).await;
        assert!(
            !w.calls().contains(&format!("close_pane {panel}")),
            "{first} {last}"
        );
    }
}

#[tokio::test]
async fn a_shell_whose_pid_changed_between_the_looks_is_not_closed() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let (_, panel) = recorded_panel(&w, dir.path()).await;
    w.s().restart_herdr();
    let mut pid = 7000;
    w.s().looks = Some(Box::new(move |_, _| {
        pid += 1;
        Shell::Plain(pid)
    }));
    w.s().ages = Some((Some(3.0), Some(false)));
    restarted(&w, dir.path(), 1, &["w1:p2"]).await;
    assert!(!w.calls().contains(&format!("close_pane {panel}")));
}

#[tokio::test]
async fn a_live_handoff_keeps_the_panels_and_rekeys_the_record() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let (mut m, panel) = recorded_panel(&w, dir.path()).await;
    for (_, ps) in &mut w.s().tabs {
        for p in ps {
            p.terminal = Some(format!(
                "{}-handoff",
                p.terminal.clone().unwrap_or("t".into())
            ));
        }
    }
    w.s().calls.clear();
    pass(&mut m, &w, &orch(BOTH), &[], false).await;
    pass(&mut m, &w, &orch(BOTH), &[], false).await;
    assert!(w.closes().is_empty());
    assert_eq!(w.order("w1:t2")[0], panel);
    let term = w.s().pane(&panel).terminal.clone().unwrap();
    assert_eq!(m.record(), [(panel, "w1:t2".to_string(), term)]);
}

#[tokio::test]
async fn a_stale_record_never_closes_a_later_pane_with_the_same_id() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    std::fs::write(
        dir.path().join("panels.json"),
        r#"{"closed": [], "panes": {"w1:p50": {"tab": "w1:t2", "terminal": "term_old"}}}"#,
    )
    .unwrap();
    let mut m = manager(&w, dir.path());
    m.started = true;
    pass(&mut m, &w, &orch(&["w1:p2"]), &[], false).await;
    assert!(m.record().is_empty());
    w.s().tab("w1:t4").insert(
        0,
        P {
            label: Some(PANEL_LABEL.into()),
            terminal: Some("term_new".into()),
            ..pane("w1:p50")
        },
    );
    pass(&mut m, &w, &orch(&["w1:p2"]), &[], false).await;
    assert!(w.closes().is_empty() && w.order("w1:t4").contains(&"w1:p50".to_string()));
}

#[tokio::test]
async fn the_panel_record_is_kept_in_the_state_dir_with_herdrs_terminal() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let (mut m, panel) = recorded_panel(&w, dir.path()).await;
    let read = || -> Value {
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("panels.json")).unwrap())
            .unwrap()
    };
    let term = w.s().pane(&panel).terminal.clone().unwrap();
    assert_eq!(
        read()["panes"],
        serde_json::json!({panel.clone(): {"tab": "w1:t2", "terminal": term}})
    );
    assert_eq!(read()["closed"], serde_json::json!([]));
    m.remove_all(Some(w.snap()), &set(&[])).await;
    assert_eq!(read()["panes"], serde_json::json!({}));
}

#[tokio::test]
async fn an_unreadable_state_file_reads_as_nothing() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    for text in [
        "not json",
        "[]",
        r#"{"closed": "w1:t2", "panes": []}"#,
        r#"{"closed": [1, "w1:t3"], "panes": {"w1:p9": 3}}"#,
    ] {
        std::fs::write(dir.path().join("panels.json"), text).unwrap();
        let m = manager(&w, dir.path());
        assert!(m.record().is_empty(), "{text}");
        assert!(m.closed().iter().all(|t| t == "w1:t3"), "{text}");
    }
}

#[tokio::test]
async fn only_a_shell_seen_young_enough_is_closed() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let (_, panel) = recorded_panel(&w, dir.path()).await;
    w.s().restart_herdr();
    w.s().shell.insert(
        panel.clone(),
        (Some(true), Some(RESTORED_SECONDS), Some(false)),
    );
    restarted(&w, dir.path(), 1, BOTH).await;
    assert!(w.calls().contains(&format!("close_pane {panel}")));
    const { assert!(RESTORED_SECONDS <= 15.0) };
}

#[tokio::test]
async fn a_pane_used_before_the_cores_first_pass_is_never_closed() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let (_, panel) = recorded_panel(&w, dir.path()).await;
    w.s().restart_herdr();
    let log = Arc::new(FocusLog::default()); // the new core's stream, connected at its start
    log.set_live(true);
    log.record(Focus::Pane(panel.clone()));
    log.record(Focus::Pane("w1:p2".into()));
    {
        let mut s = w.s();
        s.log = Some(log);
        s.focus = ("w1:t2".into(), "w1:p2".into());
        s.tab_focus.insert("w1:t2".into(), "w1:p2".into());
    }
    let again = restarted(&w, dir.path(), 1, BOTH).await;
    assert!(w.closes().is_empty() && !recorded(&again, &panel));
}

#[tokio::test]
async fn without_a_live_focus_stream_nothing_is_closed() {
    for live in [None, Some(false)] {
        let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
        let (_, panel) = recorded_panel(&w, dir.path()).await;
        w.s().restart_herdr();
        w.s().log = live.map(|_| Arc::new(FocusLog::default())); // live stays false
        let again = restarted(&w, dir.path(), RESTORE_PASSES + 1, BOTH).await;
        assert!(
            w.closes().is_empty() && !recorded(&again, &panel) && w.order("w1:t2").contains(&panel)
        );
    }
}

#[tokio::test]
async fn a_click_during_the_looks_stops_the_close() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let (_, panel) = recorded_panel(&w, dir.path()).await;
    w.s().restart_herdr();
    let (p, mut n) = (panel.clone(), 0);
    w.s().looks = Some(Box::new(move |s: &mut State, _| {
        n += 1;
        if n == 2 {
            s.record(Focus::Pane(p.clone())); // the operator clicks in during the second look
        }
        s.asked.push(format!("look {n}"));
        Shell::Plain(7000)
    }));
    w.s().ages = Some((Some(3.0), Some(false)));
    restarted(&w, dir.path(), 1, &["w1:p2"]).await;
    assert_eq!(
        w.s().asked.iter().filter(|a| a.starts_with("look")).count(),
        2
    );
    assert!(!w.calls().contains(&format!("close_pane {panel}")));
}

#[tokio::test]
async fn a_focus_stream_that_connects_within_the_passes_lets_the_core_decide() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let (_, panel) = recorded_panel(&w, dir.path()).await;
    w.s().restart_herdr();
    let log = Arc::new(FocusLog::default());
    w.s().log = Some(log.clone());
    let mut again = restarted(&w, dir.path(), 1, BOTH).await;
    assert!(w.closes().is_empty() && recorded(&again, &panel));
    log.set_live(true);
    pass(&mut again, &w, &orch(BOTH), &[], false).await;
    assert!(w.calls().contains(&format!("close_pane {panel}")));
}

#[tokio::test]
async fn a_pane_seen_focused_is_never_looked_into() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let (_, panel) = recorded_panel(&w, dir.path()).await;
    w.s().restart_herdr();
    w.s().record(Focus::Pane(panel.clone())); // the operator clicked into it after the restore
    w.s().looks = Some(Box::new(|s: &mut State, p| {
        s.asked.push(format!("look {p}"));
        Shell::Plain(7000)
    }));
    let again = restarted(&w, dir.path(), 1, BOTH).await;
    assert!(
        !w.s().asked.contains(&format!("look {panel}")),
        "its processes were asked about"
    );
    assert!(w.closes().is_empty() && !recorded(&again, &panel));
}

#[tokio::test]
async fn a_panel_left_alone_after_a_restore_is_not_recorded_again() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let (mut m, panel) = recorded_panel(&w, dir.path()).await;
    {
        let mut s = w.s();
        let p = s.pane(&panel);
        p.terminal = Some("term_new".into());
        p.agent = Some("claude".into()); // someone runs an agent in it: not our panel now
    }
    pass(&mut m, &w, &orch(BOTH), &[], false).await;
    assert!(w.closes().is_empty());
    assert!(
        !recorded(&m, &panel),
        "a pane that no longer runs our viewer was recorded"
    );
}
