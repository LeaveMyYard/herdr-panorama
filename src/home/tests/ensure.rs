//! Opening and keeping the home tab (Python `tests/test_home.py`, `tests/test_cockpit.py`).

use super::world::*;
use crate::home::{Home, Opened, ensure, open_key, startup};
use serde_json::{Map, Value, json};

fn cfg(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

fn recorded(dir: &std::path::Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(places(dir).record).unwrap()).unwrap()
}

#[tokio::test]
async fn home_opens_once_renamed_first_and_focused_on_demand() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    assert_eq!(h.tab_id, "w1:t1");
    assert_eq!(w.s().count("open_home no-focus"), 1);
    assert_eq!(w.s().label("w1:t1"), Some(label()));
    assert!(
        w.calls()
            .contains(&format!("rename_pane {} {}", h.pane_id, label()))
    );
    assert!(w.calls().contains(&"move_tab w1:t1 0".to_string()));
    assert_eq!(
        ensure(&w, &p, false).await.unwrap(),
        h,
        "never a second one"
    );
    ensure(&w, &p, true).await.unwrap();
    assert_eq!(w.s().count("open_home"), 1);
    assert_eq!(
        w.calls()
            .iter()
            .filter(|c| c.starts_with("focus_tab"))
            .collect::<Vec<_>>(),
        ["focus_tab w1:t1"]
    );
    assert_eq!(
        recorded(dir.path()),
        json!({"pane_id": h.pane_id, "tab_id": "w1:t1"})
    );
}

#[tokio::test]
async fn concurrent_openers_make_one_home() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let tasks: Vec<_> = (0..6)
        .map(|_| {
            let (w, p) = (w.clone(), p.clone());
            tokio::spawn(async move { ensure(&w, &p, false).await })
        })
        .collect();
    let mut tabs = std::collections::BTreeSet::new();
    for t in tasks {
        tabs.insert(t.await.unwrap().unwrap().tab_id);
    }
    assert_eq!(w.s().count("open_home"), 1);
    assert_eq!(tabs.into_iter().collect::<Vec<_>>(), ["w1:t1"]);
}

#[tokio::test]
async fn lost_record_adopts_the_labelled_tab_and_a_gone_home_is_reopened() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    ensure(&w, &p, false).await.unwrap();
    std::fs::remove_file(&p.record).unwrap();
    assert_eq!(
        ensure(&w, &p, false).await.unwrap().tab_id,
        "w1:t1",
        "adopted, not reopened"
    );
    assert_eq!(w.s().count("open_home"), 1);
    {
        let mut s = w.s(); // the operator closed the home tab
        s.tabs.clear();
        s.panes.clear();
    }
    assert_eq!(ensure(&w, &p, false).await.unwrap().tab_id, "w1:t2");
    assert_eq!(w.s().count("open_home"), 2);
}

#[tokio::test]
async fn open_action_focuses_home_and_the_popup_stays_available() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    assert_eq!(open_key(&w, &p, &cfg(json!({}))).await, Ok(Opened::Home));
    assert_eq!(w.s().count("open_home focus"), 1, "opened with focus");
    assert_eq!(open_key(&w, &p, &cfg(json!({}))).await, Ok(Opened::Home)); // back home
    assert_eq!(w.s().count("open_home"), 1);
    assert_eq!(
        w.calls()
            .iter()
            .filter(|c| c.starts_with("focus_tab"))
            .collect::<Vec<_>>(),
        ["focus_tab w1:t1"]
    );
    assert_eq!(w.s().count("open_popup"), 0);
}

#[tokio::test]
async fn home_tab_off_means_popup_and_no_startup_tab() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let off = cfg(json!({"home_tab": false}));
    startup(&w, &p, &off).await.unwrap();
    assert_eq!(w.s().count("open_home"), 0);
    assert_eq!(open_key(&w, &p, &off).await, Ok(Opened::Popup(None)));
    assert_eq!(w.s().count("open_popup"), 1);
}

#[tokio::test]
async fn startup_does_not_take_focus() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    startup(&w, &places(dir.path()), &cfg(json!({})))
        .await
        .unwrap();
    assert_eq!(w.s().count("open_home no-focus"), 1);
    assert_eq!(w.s().count("focus_tab"), 0);
}

#[tokio::test]
async fn a_failed_snapshot_never_opens_a_second_home() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    ensure(&w, &p, false).await.unwrap();
    let record = std::fs::read_to_string(&p.record).unwrap();
    w.s().fail.insert("snapshot");
    assert!(
        ensure(&w, &p, true).await.is_err(),
        "unknown: do not open, try again later"
    );
    assert_eq!(w.s().count("open_home"), 1);
    assert_eq!(std::fs::read_to_string(&p.record).unwrap(), record);
    let opened = open_key(&w, &p, &cfg(json!({}))).await.unwrap();
    assert!(
        matches!(opened, Opened::Popup(Some(_))),
        "the key still works: the popup"
    );
}

#[tokio::test]
async fn a_record_is_trusted_only_if_it_still_looks_like_home() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    ensure(&w, &p, false).await.unwrap();
    {
        let mut s = w.s();
        s.tabs[0].label = "work".into(); // the pane id reused by something else
        s.panes[0].agent = Some("claude".into());
        s.panes[0].argv = None;
    }
    assert_eq!(
        ensure(&w, &p, false).await.unwrap().tab_id,
        "w1:t2",
        "a new home, not the impostor"
    );
    assert_eq!(w.s().count("open_home"), 2);
}

#[tokio::test]
async fn the_home_pane_is_labelled_and_found_by_content() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    assert_eq!(
        w.calls()
            .iter()
            .filter(|c| c.starts_with("rename_pane"))
            .count(),
        1
    );
    std::fs::remove_file(&p.record).unwrap(); // the record is only a hint
    assert_eq!(ensure(&w, &p, false).await.unwrap(), h);
    assert_eq!(w.s().count("open_home"), 1);
}

#[tokio::test]
async fn the_fake_tab_focus_moves_focus() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    ensure(&w, &places(dir.path()), false).await.unwrap();
    w.s().add_agent("billing", 0);
    let (c, tc) = w.s().add_agent("docs", 0);
    crate::home::HomeOps::focus_tab(&w, &tc).await.unwrap();
    assert_eq!(w.s().focus.as_deref(), Some(c.as_str()));
}

#[tokio::test]
async fn the_key_brings_a_left_docked_panorama_home_and_focuses_it() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    let (a, ta) = w.s().add_agent("billing", 0);
    w.s().left_docked(&h.pane_id, &a);
    assert_eq!(open_key(&w, &p, &cfg(json!({}))).await, Ok(Opened::Home));
    assert_eq!(w.s().count("open_home"), 1, "no second panorama");
    let home_tab = w.s().tab_of(&h.pane_id).unwrap();
    assert_ne!(home_tab, ta);
    assert_eq!(w.s().in_tab(&ta), [a]);
    assert_eq!(
        w.calls().iter().rev().find(|c| c.starts_with("focus_tab")),
        Some(&format!("focus_tab {home_tab}"))
    );
}

#[tokio::test]
async fn closing_the_agent_tab_with_panorama_in_it_reopens_a_home() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    let (a, ta) = w.s().add_agent("billing", 0);
    w.s().left_docked(&h.pane_id, &a);
    {
        let mut s = w.s(); // the operator closed billing's tab
        s.panes.retain(|p| p.tab != ta);
        s.tabs.retain(|t| t.id != ta);
    }
    let h2 = ensure(&w, &p, false).await.unwrap();
    assert_ne!(h2.pane_id, h.pane_id);
    assert_eq!(w.s().count("open_home"), 2);
    assert_eq!(w.s().tabs[0].label, label());
}

#[tokio::test]
async fn a_panorama_left_docked_goes_back_to_its_own_first_tab() {
    // whatever the cockpit setting: ensure never reads it (Q187)
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    w.s().add_agent("search", 0);
    let (a, ta) = w.s().add_agent("billing", 0);
    w.s().left_docked(&h.pane_id, &a);
    let h2 = ensure(&w, &p, false).await.unwrap();
    assert_ne!(h2.tab_id, ta);
    assert_eq!(
        recorded(dir.path()),
        json!({"pane_id": h2.pane_id, "tab_id": h2.tab_id})
    );
    let last_move = w
        .calls()
        .into_iter()
        .rev()
        .find(|c| c.starts_with("move_pane"))
        .unwrap();
    assert_eq!(
        last_move,
        format!(
            "move_pane {} --new-tab {} --no-focus w1",
            h.pane_id,
            label()
        )
    );
    let s = w.s();
    assert_eq!(s.tabs[0].label, label());
    assert_eq!(s.in_tab(&h2.tab_id), std::slice::from_ref(&h.pane_id));
    assert_eq!(s.in_tab(&ta), [a]);
    assert!(
        s.moves_of_others(&h.pane_id).is_empty(),
        "an agent's pane was moved"
    );
}

#[tokio::test]
async fn a_failed_move_leaves_the_home_where_it_is() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    let (a, ta) = w.s().add_agent("billing", 0);
    w.s().left_docked(&h.pane_id, &a);
    w.s().fail.insert("move");
    assert_eq!(
        ensure(&w, &p, false).await.unwrap(),
        Home {
            pane_id: h.pane_id.clone(),
            tab_id: ta.clone()
        }
    );
    assert_eq!(w.s().tab_of(&h.pane_id), Some(ta));
}

#[tokio::test]
async fn an_older_home_pane_is_adopted_and_labelled() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    {
        let mut s = w.s();
        s.tabs.push(Tab {
            id: "w1:t1".into(),
            label: format!("❓2 {}", label()),
            ws: "w1".into(),
        });
        s.panes.push(Pane {
            id: "w1:p1".into(),
            tab: "w1:t1".into(),
            ws: "w1".into(),
            label: Some("Panorama".into()),
            argv: Some(VENV_PANORAMA.iter().map(|a| a.to_string()).collect()),
            ..Pane::default()
        });
        s.n = 1;
    }
    assert_eq!(
        ensure(&w, &places(dir.path()), false)
            .await
            .unwrap()
            .pane_id,
        "w1:p1"
    );
    assert_eq!(w.s().count("open_home"), 0);
    assert!(
        w.calls()
            .contains(&format!("rename_pane w1:p1 {}", label()))
    );
}

#[test]
fn records_are_per_herdr_session() {
    use herdr_agent_kit::plugin_env::PluginEnv;
    let env = |sock: &str| {
        PluginEnv::from_vars(
            [
                ("HERDR_SOCKET_PATH", sock),
                ("HERDR_PLUGIN_STATE_DIR", "/state"),
            ],
            None,
            1000,
        )
    };
    let a = crate::home::Places::from_env(&env("/run/herdr.sock"));
    let b = crate::home::Places::from_env(&env("/elsewhere/other-session.sock"));
    assert_ne!(
        a.record, b.record,
        "another `herdr --session` never reads or overwrites ours"
    );
    assert_ne!(a.lock, b.lock);
    assert!(a.record.starts_with("/state") && a.record.extension().is_some_and(|e| e == "json"));
    assert_eq!(a.lock.parent(), a.record.parent());
}

#[tokio::test]
async fn ensure_with_the_lock_held_elsewhere_gives_up() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let mut p = places(dir.path());
    p.wait = std::time::Duration::from_millis(200);
    let held = herdr_agent_kit::plugin_env::lock::LockFile::new(&p.lock)
        .try_exclusive()
        .unwrap()
        .unwrap();
    let t0 = std::time::Instant::now();
    let err = ensure(&w, &p, true).await.unwrap_err();
    assert!(
        err.contains("holds the home lock") && t0.elapsed() < std::time::Duration::from_secs(1)
    );
    assert_eq!(w.s().count("open_home"), 0);
    drop(held);
    assert!(ensure(&w, &p, true).await.is_ok());
}

#[test]
fn the_record_and_lock_are_where_python_keeps_them() {
    // a Python and a Rust panorama must share one record and one lock, or both would make a
    // home: `home._record()` and `home._lock()` (values from home.py itself)
    use herdr_agent_kit::plugin_env::PluginEnv;
    let env = PluginEnv::from_vars(
        [
            ("HERDR_PLUGIN_STATE_DIR", "/state"),
            ("HERDR_SOCKET_PATH", "/run/herdr.sock"),
        ],
        None,
        1000,
    );
    let p = crate::home::Places::from_env(&env);
    assert_eq!(
        p.record,
        std::path::Path::new("/state/home-837fc5258150.json")
    );
    assert_eq!(
        p.lock,
        std::path::Path::new("/state/home-837fc5258150.lock")
    );
    let env = PluginEnv::from_vars([("XDG_STATE_HOME", "/xdg")], None, 1000);
    let p = crate::home::Places::from_env(&env);
    assert_eq!(
        p.record,
        std::path::Path::new("/xdg/herdr-panorama/home-7505d64a54e0.json")
    );
    assert_eq!(
        p.lock,
        std::path::Path::new("/xdg/herdr-panorama/home-7505d64a54e0.lock")
    );
    assert_eq!(p.wait, crate::home::LOCK);
}

#[tokio::test]
async fn a_docked_home_goes_back_home_in_its_own_workspace() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    {
        // an agent's tab in workspace w2, the home left docked beside it there
        let mut s = w.s();
        s.tabs.push(Tab {
            id: "w2:t7".into(),
            label: "billing".into(),
            ws: "w2".into(),
        });
        s.panes.push(Pane {
            id: "w2:p7".into(),
            tab: "w2:t7".into(),
            ws: "w2".into(),
            agent: Some("claude".into()),
            ..Pane::default()
        });
        s.left_docked(&h.pane_id, "w2:p7");
        s.pane(&h.pane_id).ws = "w2".into();
    }
    let h2 = ensure(&w, &p, false).await.unwrap();
    let last_move = w
        .calls()
        .into_iter()
        .rev()
        .find(|c| c.starts_with("move_pane"))
        .unwrap();
    assert!(last_move.ends_with(" w2"), "{last_move}");
    assert!(h2.tab_id.starts_with("w2:"), "{h2:?}");
    assert_eq!(
        h2.pane_id, h.pane_id,
        "within its workspace the pane keeps its id"
    );
    assert_eq!(w.s().in_tab("w2:t7"), ["w2:p7"]);
}
