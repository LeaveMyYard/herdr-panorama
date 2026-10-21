//! Issue #9: a `◆ panorama` tab never keeps that label once panorama is not in it, after a
//! quit or a crash (Python `tests/test_home_label.py`).

use super::world::*;
use crate::home::label::is_home_label;
use crate::home::{ensure, leave};

#[test]
fn the_home_label_also_behind_inbox_marks() {
    assert!(is_home_label("◆ panorama"));
    assert!(is_home_label("❓2 ◆ panorama"));
    assert!(is_home_label("❓2 ❓13  ◆ panorama"));
    assert!(!is_home_label("❓◆ panorama"), "a mark needs its number");
    assert!(
        !is_home_label("❓ ◆ panorama"),
        "a number, not only a space"
    );
    assert!(!is_home_label("❓2◆ panorama"), "and a space");
    assert!(!is_home_label("◆ panorama ❓2"));
    assert!(!is_home_label("work"));
    assert!(!is_home_label(""));
}

#[tokio::test]
async fn crash_sweep_renames_a_home_tab_left_with_an_agent() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    let (_, tab) = w.s().beside_home(&h.pane_id, true); // the operator split it and started Claude
    w.s().drop_pane(&h.pane_id); // panorama crashed: its pane is gone
    let h2 = ensure(&w, &p, false).await.unwrap();
    assert_eq!(
        w.s().label(&tab).as_deref(),
        Some("billing"),
        "the agent's name, never ◆ panorama"
    );
    assert_ne!(h2.tab_id, tab);
    assert_eq!(w.s().label(&h2.tab_id), Some(label()));
}

#[tokio::test]
async fn sweep_falls_back_to_the_kind_then_tab() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    let (other, tab) = w.s().beside_home(&h.pane_id, true);
    w.s().pane(&other).name = None;
    w.s().drop_pane(&h.pane_id);
    ensure(&w, &p, false).await.unwrap();
    assert_eq!(w.s().label(&tab).as_deref(), Some("claude"));
    let h3 = ensure(&w, &p, false).await.unwrap();
    w.s().panes.retain(|p| p.id != "w1:p50");
    let (_, tab2) = w.s().beside_home(&h3.pane_id, false);
    w.s().drop_pane(&h3.pane_id);
    ensure(&w, &p, false).await.unwrap();
    assert_eq!(w.s().label(&tab2).as_deref(), Some("tab"));
}

#[tokio::test]
async fn sweep_keeps_a_live_or_unverifiable_panorama_tab() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    w.s().beside_home(&h.pane_id, true);
    ensure(&w, &p, false).await.unwrap();
    assert_eq!(
        w.s().label(&h.tab_id),
        Some(label()),
        "panorama is still in it"
    );
    w.s().pane(&h.pane_id).argv = Some(vec![]); // mid-restore: unknown
    let _ = ensure(&w, &p, false).await;
    assert_eq!(
        w.s().label(&h.tab_id),
        Some(label()),
        "could be panorama: never renamed"
    );
    assert!(
        !w.calls()
            .iter()
            .any(|c| c.starts_with(&format!("rename_tab {} ", h.tab_id)) && !c.ends_with(&label()))
    );
}

#[tokio::test]
async fn sweep_ignores_other_tabs() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    ensure(&w, &p, false).await.unwrap();
    {
        let mut s = w.s();
        s.tabs.push(Tab {
            id: "w1:t8".into(),
            label: "work".into(),
            ws: "w1".into(),
        });
        s.panes.push(Pane {
            id: "w1:p8".into(),
            tab: "w1:t8".into(),
            ws: "w1".into(),
            ..Pane::default()
        });
    }
    ensure(&w, &p, false).await.unwrap();
    assert_eq!(w.s().label("w1:t8").as_deref(), Some("work"));
}

#[tokio::test]
async fn quit_renames_the_shared_home_tab_first() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    let (_, tab) = w.s().beside_home(&h.pane_id, true);
    leave(&w, &p, &h.pane_id).await.unwrap();
    assert_eq!(w.s().label(&tab).as_deref(), Some("billing"));
}

#[tokio::test]
async fn quit_alone_in_the_home_tab_renames_nothing() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    let before = w.s().count("rename_tab");
    leave(&w, &p, &h.pane_id).await.unwrap();
    assert_eq!(
        w.s().label(&h.tab_id),
        Some(label()),
        "the tab closes with panorama's pane"
    );
    assert_eq!(w.s().count("rename_tab"), before);
}

#[tokio::test]
async fn quit_never_waits_long_for_a_busy_lock() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    w.s().beside_home(&h.pane_id, true);
    let held = herdr_agent_kit::plugin_env::lock::LockFile::new(&p.lock)
        .try_exclusive()
        .unwrap()
        .unwrap();
    let t0 = std::time::Instant::now();
    assert!(
        leave(&w, &p, &h.pane_id)
            .await
            .unwrap_err()
            .contains("busy")
    );
    assert!(t0.elapsed() < std::time::Duration::from_secs(2));
    assert_eq!(
        w.s().label(&h.tab_id),
        Some(label()),
        "skipped: the next start sweeps it"
    );
    drop(held);
}

#[tokio::test]
async fn sweep_renames_even_when_an_agents_process_is_unknown() {
    // only agent-less panes can be panorama: an agent whose process herdr cannot describe
    // does not keep the ◆ label
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    let (other, tab) = w.s().beside_home(&h.pane_id, true);
    w.s().pane(&other).argv = Some(vec![]);
    w.s().drop_pane(&h.pane_id);
    ensure(&w, &p, false).await.unwrap();
    assert_eq!(w.s().label(&tab).as_deref(), Some("billing"));
}

#[tokio::test]
async fn quit_from_a_tab_that_is_no_home_tab_renames_nothing() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    let (a, ta) = w.s().add_agent("billing", 0);
    w.s().left_docked(&h.pane_id, &a);
    leave(&w, &p, &h.pane_id).await.unwrap();
    assert_eq!(w.s().label(&ta).as_deref(), Some("billing"));
    assert_eq!(
        w.calls()
            .iter()
            .filter(|c| c.starts_with(&format!("rename_tab {ta}")))
            .count(),
        0
    );
}
