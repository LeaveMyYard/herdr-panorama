//! Look-alikes and the settings (Python `tests/test_cockpit.py`: the reviews of #8).

use super::world::*;
use crate::home::config::{arrows, follow_tabs_single, home_enabled, keep_focus, per_tab};
use crate::home::ensure;
use serde_json::{Map, Value, json};

fn cfg(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

fn argv(a: &[&str]) -> Option<Vec<String>> {
    Some(a.iter().map(|x| x.to_string()).collect())
}

/// A look-alike first in the tab order (it wins on order alone): its pane.
fn lookalike(s: &mut State, pane_label: Option<&str>, tab_label: &str, a: &[&str]) -> String {
    s.n += 1;
    let (t, p) = (format!("w1:t{}", s.n), format!("w1:p{}", s.n));
    s.tabs.insert(
        0,
        Tab {
            id: t.clone(),
            label: tab_label.into(),
            ws: "w1".into(),
        },
    );
    let pane = Pane {
        id: p.clone(),
        tab: t,
        ws: "w1".into(),
        label: pane_label.map(Into::into),
        argv: argv(a),
        ..Pane::default()
    };
    s.panes.insert(0, pane);
    p
}

#[tokio::test]
async fn lookalikes_are_never_adopted_renamed_moved_or_closed() {
    let home = label();
    let cases: [(&str, Option<&str>, &str, &[&str]); 5] = [
        ("shell with the label", Some(&home), "x", &["zsh"]),
        (
            "panorama exited, its shell left",
            Some(&home),
            &home,
            &["zsh"],
        ),
        (
            "another plugin alone in a ◆ tab",
            None,
            &home,
            &["uv", "run", "--project", "/plugins/other", "other", "tui"],
        ),
        (
            "label restored without its process",
            Some(&home),
            "x",
            &["sleep", "900"],
        ),
        ("an agent that just exited", None, &home, &["zsh"]),
    ];
    for (kind, pane_label, tab_label, a) in cases {
        let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
        let p = places(dir.path());
        let h = ensure(&w, &p, false).await.unwrap();
        let fake = lookalike(&mut w.s(), pane_label, tab_label, a);
        std::fs::remove_file(&p.record).unwrap(); // no tie-breaker: order alone would pick it
        assert_eq!(ensure(&w, &p, false).await.unwrap(), h, "{kind}");
        let (agent, _) = w.s().add_agent("billing", 0);
        w.s().left_docked(&h.pane_id, &agent);
        ensure(&w, &p, false).await.unwrap(); // panorama goes home; nothing else moves
        let calls = w.calls();
        assert!(
            !calls.iter().any(
                |c| (c.starts_with("rename_pane ") || c.starts_with("move_pane "))
                    && c.contains(&fake)
            ),
            "{kind}: {calls:?}"
        );
        assert!(w.s().tab_of(&fake).is_some(), "{kind}: untouched");
        assert!(w.s().moves_of_others(&h.pane_id).is_empty(), "{kind}");
    }
}

#[tokio::test]
async fn only_a_lookalike_left_means_a_new_panorama_not_an_adoption() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    w.s().pane(&h.pane_id).argv = argv(&["zsh"]); // panorama quit; its shell keeps the label
    let h2 = ensure(&w, &p, false).await.unwrap();
    assert_ne!(h2.pane_id, h.pane_id);
    assert_eq!(w.s().count("open_home"), 2);
}

#[tokio::test]
async fn unverifiable_lookalikes_open_nothing() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    ensure(&w, &p, false).await.unwrap();
    w.s().fail.insert("process_info");
    assert!(ensure(&w, &p, false).await.unwrap_err().contains("verify"));
    assert_eq!(w.s().count("open_home"), 1);
    let (code, err) = open_key(&w, &p, &cfg(json!({}))).await;
    assert_eq!(code, 0, "the key still works: the popup");
    assert!(
        err.contains("verify") && err.ends_with("; opening the popup\n"),
        "{err}"
    );
    assert_eq!(w.s().count("open_view split=false"), 1);
    assert_eq!(w.s().count("open_home"), 1, "never a second home");
}

#[tokio::test]
async fn the_process_match_is_panorama_then_tui() {
    for (a, runs) in [
        (PANORAMA, Some(true)),
        (VENV_PANORAMA, Some(true)),
        (&["panorama", "tui"][..], Some(true)),
        (&["zsh"][..], Some(false)),
        (&["panorama", "snapshot"][..], Some(false)),
        (&["uv", "run", "other", "tui"][..], Some(false)),
        (&["vim", "panorama.md"][..], Some(false)),
        (&[][..], None), // no foreground process: unknown
    ] {
        let w = World::default();
        let p = w.s().add_agent("x", 0).0;
        {
            let mut s = w.s();
            let pane = s.pane(&p);
            (pane.agent, pane.argv) = (None, argv(a));
        }
        assert_eq!(crate::home::HomeOps::runs_tui(&w, &p).await, runs, "{a:?}");
    }
}

#[tokio::test]
async fn a_shared_home_tab_is_left_alone() {
    // whatever the cockpit setting: ensure never moves a home out of its own (split) home tab
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    let (_, tab) = w.s().beside_home(&h.pane_id, true);
    assert_eq!(ensure(&w, &p, false).await.unwrap(), h);
    assert_eq!(w.s().count("move_pane"), 0);
    assert_eq!(w.s().tab_of(&h.pane_id), Some(tab));
}

#[tokio::test]
async fn duplicates_are_left_alone_not_closed() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    {
        let mut s = w.s();
        s.tabs.push(Tab {
            id: "w1:t9".into(),
            label: "x".into(),
            ws: "w1".into(),
        });
        s.panes.push(Pane {
            id: "w1:p9".into(),
            tab: "w1:t9".into(),
            ws: "w1".into(),
            label: Some(label()),
            argv: argv(PANORAMA),
            ..Pane::default()
        });
    }
    assert_eq!(ensure(&w, &p, false).await.unwrap(), h);
    assert_eq!(w.s().tab_of("w1:p9").as_deref(), Some("w1:t9"));
    assert!(
        !w.calls()
            .iter()
            .any(|c| c.contains("w1:p9") && !c.starts_with("process_info"))
    );
}

#[test]
fn config_defaults_and_python_truthiness() {
    assert!(home_enabled(&cfg(json!({}))) && !per_tab(&cfg(json!({}))));
    assert!(per_tab(&cfg(json!({"cockpit": true}))));
    assert!(per_tab(&cfg(json!({"cockpit": "on"}))) && per_tab(&cfg(json!({"cockpit": 1}))));
    assert!(!per_tab(&cfg(json!({"cockpit": "nope"}))) && !per_tab(&cfg(json!({"cockpit": 1.0}))));
    assert!(
        !per_tab(&cfg(json!({"cockpit": true, "home_tab": false}))),
        "the cockpit starts from the home pane"
    );
    assert!(
        !home_enabled(&cfg(json!({"home_tab": "OFF"})))
            && home_enabled(&cfg(json!({"home_tab": "maybe"})))
    );
    assert!(!home_enabled(&cfg(json!({"home_tab": 0}))));
    // an old `cockpit_panels = "moving"` reads as per-tab (Q187)
    assert!(per_tab(&cfg(
        json!({"cockpit": true, "cockpit_panels": "moving"})
    )));
    use crate::panels::fit::width;
    assert_eq!(width(None), 46);
    assert_eq!(width(Some(&json!(500))), 119);
    assert_eq!(width(Some(&json!(20))), 36);
}

#[test]
fn dock_keep_focus_and_arrows_default_on() {
    assert!(keep_focus(&cfg(json!({}))) && arrows(&cfg(json!({}))));
    assert!(!keep_focus(&cfg(json!({"dock_keep_focus": false}))));
    assert!(
        !arrows(&cfg(json!({"dock_keep_focus": false}))),
        "arrows need keep-focus"
    );
    assert!(!arrows(&cfg(json!({"dock_on_arrows": "no"}))));
    assert!(!follow_tabs_single(&cfg(json!({}))));
    assert!(follow_tabs_single(&cfg(
        json!({"cockpit_follow_tabs": "Single"})
    )));
    assert!(!follow_tabs_single(&cfg(
        json!({"cockpit_follow_tabs": "nonsense"})
    )));
}

#[tokio::test]
async fn the_callers_own_pane_wins_then_the_record_then_the_first() {
    let (w, dir) = (World::default(), tempfile::tempdir().unwrap());
    let p = places(dir.path());
    let h = ensure(&w, &p, false).await.unwrap();
    {
        let mut s = w.s(); // a second verified panorama, first in herdr's order
        s.tabs.insert(
            0,
            Tab {
                id: "w1:t9".into(),
                label: "x".into(),
                ws: "w1".into(),
            },
        );
        s.panes.insert(
            0,
            Pane {
                id: "w1:p9".into(),
                tab: "w1:t9".into(),
                ws: "w1".into(),
                label: Some(label()),
                argv: argv(PANORAMA),
                ..Pane::default()
            },
        );
    }
    let found = |prefer: Option<&'static str>| {
        let (w, p) = (w.clone(), p.clone());
        async move {
            crate::home::find_home(&w, &p, prefer)
                .await
                .unwrap()
                .pane_id
        }
    };
    assert_eq!(found(None).await, h.pane_id, "the record");
    assert_eq!(found(Some("w1:p9")).await, "w1:p9", "the caller's own");
    let rec = std::fs::read_to_string(&p.record).unwrap();
    assert!(
        rec.contains("\"w1:p9\""),
        "the record follows the pane used: {rec}"
    );
    std::fs::remove_file(&p.record).unwrap();
    assert_eq!(found(None).await, "w1:p9", "no record: the first");
    assert_eq!(
        found(Some("w1:p77")).await,
        "w1:p9",
        "a preferred pane that is not panorama: ignored"
    );
}

#[test]
fn what_looks_like_the_home_pane() {
    use crate::home::lookalikes;
    use crate::panels::snap::{Snap, SnapPane, SnapTab};
    let pane = |id: &str, tab: &str, label: Option<&str>, agent: Option<&str>| SnapPane {
        pane_id: id.into(),
        tab_id: Some(tab.into()),
        label: label.map(Into::into),
        agent: agent.map(Into::into),
        ..SnapPane::default()
    };
    let tab = |id: &str, label: &str| SnapTab {
        tab_id: id.into(),
        label: Some(label.into()),
        workspace_id: None,
    };
    let home = label();
    let snap = Snap {
        panes: vec![
            pane("w1:p1", "w1:t1", Some(&home), None), // labelled: a look-alike
            pane("w1:p2", "w1:t2", Some(&home), Some("claude")), // labelled, but an agent
            pane("w1:p3", "w1:t3", None, None),        // the only agent-less pane of a ◆ tab
            pane("w1:p4", "w1:t3", None, Some("claude")),
            pane("w1:p5", "w1:t4", None, None), // two agent-less panes in a ◆ tab
            pane("w1:p6", "w1:t4", None, None),
            pane("w1:p7", "w1:t5", None, None), // alone in a tab that is no home tab
        ],
        tabs: vec![
            tab("w1:t1", "x"),
            tab("w1:t2", "y"),
            tab("w1:t3", &format!("❓1 {home}")),
            tab("w1:t4", &home),
            tab("w1:t5", "work"),
        ],
        ..Snap::default()
    };
    let ids: Vec<String> = lookalikes(&snap).into_iter().map(|p| p.pane_id).collect();
    assert_eq!(ids, ["w1:p1", "w1:p3"]);
}
