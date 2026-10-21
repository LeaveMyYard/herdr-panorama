//! What is ours to close, closed stays closed, the open key, orphans and crashes (Python
//! `tests/test_panels.py`).

use super::world::*;
use crate::panels::{CRASH_LIMIT, OpenKey, REMOVE_AFTER};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[tokio::test]
async fn a_panel_the_operator_closed_stays_closed_until_the_open_key() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let mut m = manager(&w, dir.path());
    let sw = orch(&["w1:p2"]);
    pass(&mut m, &w, &sw, &[], true).await;
    let panel = w.order("w1:t2")[0].clone();
    pass(&mut m, &w, &sw, &[&panel], false).await; // its viewer connected
    *w.s().tab("w1:t2") = vec![agent("w1:p2")]; // the operator closed it
    w.s().focus = ("w1:t2".into(), "w1:p2".into());
    w.s().calls.clear();
    pass(&mut m, &w, &sw, &[], false).await;
    pass(&mut m, &w, &sw, &[], false).await;
    assert!(w.calls().is_empty());
    assert_eq!(m.closed(), &set(&["w1:t2"]));
    let mut again = manager(&w, dir.path()); // a restarted core remembers
    again.started = true;
    pass(&mut again, &w, &sw, &[], false).await;
    assert!(w.calls().is_empty());
    assert_eq!(
        again
            .open_key(&w.snap(), &sw, Some(&home()), &set(&[]), false)
            .await,
        OpenKey::Panel
    );
    let new = w.order("w1:t2")[0].clone();
    assert_ne!(new, panel);
    assert_eq!(w.s().focus, ("w1:t2".into(), new));
    assert!(again.closed().is_empty());
    assert!(manager(&w, dir.path()).closed().is_empty(), "saved");
}

#[tokio::test]
async fn the_open_key_focuses_an_existing_panel_or_says_home() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let mut m = manager(&w, dir.path());
    let sw = orch(&["w1:p2"]);
    pass(&mut m, &w, &sw, &[], true).await;
    w.s().focus = ("w1:t2".into(), "w1:p2".into());
    let opens = w.calls().iter().filter(|c| c.starts_with("open")).count();
    assert_eq!(
        m.open_key(&w.snap(), &sw, Some(&home()), &set(&[]), false)
            .await,
        OpenKey::Panel
    );
    let first = w.order("w1:t2")[0].clone();
    assert_eq!(w.s().focus, ("w1:t2".into(), first));
    assert_eq!(
        w.calls().iter().filter(|c| c.starts_with("open")).count(),
        opens,
        "no second panel"
    );
    w.s().focus = ("w1:t3".into(), "w1:p3".into()); // not an orchestrator of the model
    assert_eq!(
        m.open_key(&w.snap(), &sw, Some(&home()), &set(&[]), false)
            .await,
        OpenKey::Home
    );
}

#[tokio::test]
async fn the_open_key_waits_for_the_home_lock_then_says_busy() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let lock = dir.path().join("home.lock");
    let mut p = places(dir.path());
    p.home_lock = lock.clone();
    let mut m = crate::panels::Manager::new(w.clone(), None, p, Arc::new(Instant::now));
    let held = herdr_agent_kit::plugin_env::lock::LockFile::new(&lock)
        .try_exclusive()
        .unwrap()
        .unwrap();
    let t0 = Instant::now();
    assert_eq!(
        m.open_key(
            &w.snap(),
            &orch(&["w1:p2"]),
            Some(&home()),
            &set(&[]),
            false
        )
        .await,
        OpenKey::Busy
    );
    assert!(t0.elapsed() >= crate::panels::LOCK);
    assert!(
        m.reconcile(
            w.snap(),
            &orch(&["w1:p2"]),
            Some(&home()),
            &set(&[]),
            true,
            false
        )
        .await
        .is_none()
    );
    assert!(w.calls().is_empty());
    drop(held);
    assert!(
        m.reconcile(
            w.snap(),
            &orch(&["w1:p2"]),
            Some(&home()),
            &set(&[]),
            true,
            false
        )
        .await
        .is_some()
    );
    assert!(!w.calls().is_empty());
}

#[tokio::test]
async fn only_provable_panels_are_closed() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    w.s().tab("w1:t2").insert(0, pane("w1:p50"));
    w.s().viewer("w1:p50");
    let mut m = manager(&w, dir.path());
    pass(&mut m, &w, &orch(&["w1:p2"]), &[], true).await;
    assert!(
        !w.calls().contains(&"open w1:p2".to_string()),
        "it still counts as the tab's panel"
    );
    assert_eq!(m.remove_all(Some(w.snap()), &set(&[])).await, 0);
    assert!(w.order("w1:t2").contains(&"w1:p50".to_string()));
    let mut verified = manager(&w, dir.path());
    assert_eq!(verified.remove_all(None, &set(&["w1:p50"])).await, 1);
    assert!(!w.order("w1:t2").contains(&"w1:p50".to_string()));
}

#[tokio::test]
async fn remove_all_leaves_agents_and_panes_not_running_the_viewer() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let mut m = manager(&w, dir.path());
    // plugin panes herdr would close, but one runs an agent and one no longer our viewer
    w.s()
        .plugin
        .extend(["w1:p1".to_string(), "w1:p2".to_string()]);
    assert_eq!(m.remove_all(None, &set(&["w1:p1", "w1:p2"])).await, 0);
    assert!(w.closes().is_empty());
}

#[tokio::test]
async fn a_second_panel_is_closed_keeping_the_connected_one() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let mut m = manager(&w, dir.path());
    pass(&mut m, &w, &orch(&["w1:p2"]), &[], true).await;
    let first = w.order("w1:t2")[0].clone();
    w.s().tab("w1:t2").push(pane("w1:p70"));
    w.s().viewer("w1:p70");
    pass(&mut m, &w, &orch(&["w1:p2"]), &["w1:p70"], false).await;
    assert!(w.calls().contains(&format!("close {first}")));
    assert!(w.order("w1:t2").contains(&"w1:p70".to_string()));
    assert!(!m.created.contains_key(&first));
}

#[tokio::test]
async fn a_second_look_alike_panel_is_left_but_not_counted() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    for p in ["w1:p50", "w1:p51"] {
        w.s().tab("w1:t2").insert(0, pane(p));
        w.s().viewer(p);
    }
    let mut m = manager(&w, dir.path());
    let sv = m
        .reconcile(
            w.snap(),
            &orch(&["w1:p2"]),
            Some(&home()),
            &set(&[]),
            true,
            false,
        )
        .await
        .unwrap();
    assert!(w.closes().is_empty());
    assert_eq!(sv.panels["w1:t2"].len(), 1);
}

#[tokio::test]
async fn a_tab_without_its_orchestrator_loses_the_panel_only_after_a_while() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let now = Arc::new(Mutex::new(Instant::now()));
    let c = now.clone();
    let mut m = manager_with(&w, dir.path(), Arc::new(move || *c.lock().unwrap()));
    let tick = |d: std::time::Duration| *now.lock().unwrap() += d;
    let one = std::time::Duration::from_secs(1);
    pass(&mut m, &w, &orch(&["w1:p2"]), &[], true).await;
    let panel = w.order("w1:t2")[0].clone();
    pass(&mut m, &w, &orch(&[]), &[], false).await; // Claude exited
    tick(REMOVE_AFTER - one);
    pass(&mut m, &w, &orch(&[]), &[], false).await;
    assert!(w.order("w1:t2").contains(&panel));
    pass(&mut m, &w, &orch(&["w1:p2"]), &[], false).await; // started again in time: kept
    tick(REMOVE_AFTER + one);
    pass(&mut m, &w, &orch(&["w1:p2"]), &[], false).await;
    assert!(w.order("w1:t2").contains(&panel));
    pass(&mut m, &w, &orch(&[]), &[], false).await;
    tick(REMOVE_AFTER);
    pass(&mut m, &w, &orch(&[]), &[], false).await;
    assert!(!w.order("w1:t2").contains(&panel));
    assert!(w.calls().contains(&format!("close {panel}")));
}

#[tokio::test]
async fn a_panel_that_dies_before_its_viewer_connected_is_a_crash_not_a_close() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let mut m = manager(&w, dir.path());
    let sw = orch(&["w1:p2"]);
    pass(&mut m, &w, &sw, &[], true).await;
    w.s().focus = ("w1:t2".into(), "w1:p2".into());
    for _ in 0..CRASH_LIMIT + 2 {
        let panel = w.order("w1:t2")[0].clone();
        if !w.s().argv.contains_key(&panel) {
            break;
        }
        *w.s().tab("w1:t2") = vec![agent("w1:p2")]; // it died
        pass(&mut m, &w, &sw, &[], false).await;
    }
    assert!(m.closed().is_empty());
    let opens = w.calls().iter().filter(|c| *c == "open w1:p2").count();
    assert_eq!(
        opens as u32, CRASH_LIMIT,
        "the first one and the re-makes, then it gives up"
    );
    assert!(m.broken.contains("w1:t2"));
    assert_eq!(m.crashes["w1:t2"], CRASH_LIMIT);
}

#[tokio::test]
async fn a_panel_whose_viewer_connected_then_vanished_was_closed_by_the_operator() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let mut m = manager(&w, dir.path());
    let sw = orch(&["w1:p2"]);
    pass(&mut m, &w, &sw, &[], true).await;
    let panel = w.order("w1:t2")[0].clone();
    pass(&mut m, &w, &sw, &[&panel], false).await; // its viewer is connected
    *w.s().tab("w1:t2") = vec![agent("w1:p2")];
    pass(&mut m, &w, &sw, &[], false).await;
    assert_eq!(m.closed(), &set(&["w1:t2"]));
}

#[tokio::test]
async fn a_closed_mark_goes_with_its_tab() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let mut m = manager(&w, dir.path());
    let sw = orch(&["w1:p2"]);
    pass(&mut m, &w, &sw, &[], true).await;
    let panel = w.order("w1:t2")[0].clone();
    pass(&mut m, &w, &sw, &[&panel], false).await;
    *w.s().tab("w1:t2") = vec![agent("w1:p2")];
    pass(&mut m, &w, &sw, &[], false).await;
    w.s().tabs.retain(|(t, _)| t != "w1:t2");
    pass(&mut m, &w, &sw, &[], false).await;
    assert!(m.closed().is_empty());
    assert!(manager(&w, dir.path()).closed().is_empty(), "saved");
}

#[tokio::test]
async fn a_crash_marker_wins_over_a_connected_viewer() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let mut m = manager(&w, dir.path());
    let sw = orch(&["w1:p2"]);
    pass(&mut m, &w, &sw, &[], true).await;
    let p = w.order("w1:t2")[0].clone();
    pass(&mut m, &w, &sw, &[&p], false).await;
    let state = dir.path().join("state");
    std::fs::create_dir_all(&state).unwrap();
    crate::ui::crash::mark(&state, Some(&p), "boom", "now"); // died of an error after connecting
    assert!(crate::ui::crash::marker(&state, &p).exists());
    *w.s().tab("w1:t2") = vec![agent("w1:p2")];
    pass(&mut m, &w, &sw, &[], false).await;
    assert!(m.closed().is_empty());
    assert!(!crate::ui::crash::marker(&state, &p).exists(), "used up");
}

#[tokio::test]
async fn an_orphaned_look_alike_is_never_closed() {
    // a look-alike left of our panel in a tab whose orchestrator went, our panel's close
    // refused: the tab stays surveyed through our panel, and the look-alike is what is left
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let now = Arc::new(Mutex::new(Instant::now()));
    let c = now.clone();
    let mut m = manager_with(&w, dir.path(), Arc::new(move || *c.lock().unwrap()));
    pass(&mut m, &w, &orch(&["w1:p2"]), &[], true).await;
    let ours = w.order("w1:t2")[0].clone();
    w.s().tab("w1:t2").insert(0, pane("w1:p50"));
    w.s().viewer("w1:p50");
    w.s().fail_close.insert(ours.clone());
    pass(&mut m, &w, &orch(&[]), &[], false).await; // its orchestrator went
    *now.lock().unwrap() += REMOVE_AFTER;
    pass(&mut m, &w, &orch(&[]), &[], false).await;
    assert!(!w.calls().contains(&"close w1:p50".to_string()));
    assert!(w.order("w1:t2").contains(&"w1:p50".to_string()));
}

#[tokio::test]
async fn an_orchestrator_that_comes_back_restarts_the_orphan_wait() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let now = Arc::new(Mutex::new(Instant::now()));
    let c = now.clone();
    let mut m = manager_with(&w, dir.path(), Arc::new(move || *c.lock().unwrap()));
    pass(&mut m, &w, &orch(&["w1:p2"]), &[], true).await;
    let panel = w.order("w1:t2")[0].clone();
    pass(&mut m, &w, &orch(&[]), &[], false).await; // it left
    *now.lock().unwrap() += REMOVE_AFTER;
    pass(&mut m, &w, &orch(&["w1:p2"]), &[], false).await; // back just in time
    pass(&mut m, &w, &orch(&[]), &[], false).await; // left again: a new wait
    assert!(w.order("w1:t2").contains(&panel), "closed on the old wait");
}

#[tokio::test]
async fn a_panel_herdr_would_not_open_is_not_asked_for_again_at_once() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let now = Arc::new(Mutex::new(Instant::now()));
    let c = now.clone();
    let mut m = manager_with(&w, dir.path(), Arc::new(move || *c.lock().unwrap()));
    m.started = true;
    w.s().fail_open = Some("ui_busy".into());
    w.s().focus = ("w1:t2".into(), "w1:p2".into());
    let sw = orch(&["w1:p2"]);
    pass(&mut m, &w, &sw, &[], false).await;
    pass(&mut m, &w, &sw, &[], false).await;
    let opens = || w.calls().iter().filter(|c| *c == "open w1:p2").count();
    assert_eq!(opens(), 1, "CREATING: not again at once");
    w.s().fail_open = None;
    *now.lock().unwrap() += crate::panels::CREATING;
    pass(&mut m, &w, &sw, &[], false).await;
    assert_eq!(opens(), 2);
    let first = w.order("w1:t2")[0].clone();
    assert!(w.s().argv.contains_key(&first));
}

#[tokio::test]
async fn a_restarted_core_adopts_a_verified_panel() {
    // adopted: its viewer said a verified hello, so it is ours (closing it by hand is closed)
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let mut m = manager(&w, dir.path());
    let sw = orch(&["w1:p2"]);
    pass(&mut m, &w, &sw, &[], true).await;
    let panel = w.order("w1:t2")[0].clone();
    let mut again = manager(&w, dir.path());
    again.started = true;
    pass(&mut again, &w, &sw, &[&panel], false).await;
    assert_eq!(again.created.get(&panel).map(String::as_str), Some("w1:t2"));
    *w.s().tab("w1:t2") = vec![agent("w1:p2")]; // the operator closed it
    pass(&mut again, &w, &sw, &[], false).await;
    assert_eq!(again.closed(), &set(&["w1:t2"]));
}

#[tokio::test]
async fn a_crash_marker_that_cannot_be_removed_still_counts_as_a_crash() {
    // Python: an OSError other than "not found" on the marker is a crash too
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let mut m = manager(&w, dir.path());
    let sw = orch(&["w1:p2"]);
    pass(&mut m, &w, &sw, &[], true).await;
    let p = w.order("w1:t2")[0].clone();
    pass(&mut m, &w, &sw, &[&p], false).await;
    let marker = crate::ui::crash::marker(&dir.path().join("state"), &p);
    std::fs::create_dir_all(&marker).unwrap(); // a directory: remove_file fails, not NotFound
    *w.s().tab("w1:t2") = vec![agent("w1:p2")];
    pass(&mut m, &w, &sw, &[], false).await;
    assert!(m.closed().is_empty());
    assert_eq!(m.crashes["w1:t2"], 1);
}

#[tokio::test]
async fn remove_all_never_closes_a_pane_running_an_agent() {
    // our panel's pane now holds an agent (herdr detected one there): not a panel to close
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let mut m = manager(&w, dir.path());
    pass(&mut m, &w, &orch(&["w1:p2"]), &[], true).await;
    let panel = w.order("w1:t2")[0].clone();
    w.s().pane(&panel).agent = Some("claude".into());
    assert_eq!(m.remove_all(None, &set(&[&panel])).await, 0);
    assert!(w.closes().is_empty());
}
