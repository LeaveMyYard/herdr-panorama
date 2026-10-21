//! The start batch, the operator's tab afterwards, and the lazy placement (Python
//! `tests/test_panels.py`).

use super::world::*;
use crate::panels::{Focus, operator_moved};

fn t(x: &str) -> Focus {
    Focus::Tab(x.into())
}
fn p(x: &str) -> Focus {
    Focus::Pane(x.into())
}

#[tokio::test]
async fn the_start_batch_puts_a_panel_left_of_every_orchestrator_and_the_operator_back() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let mut m = manager(&w, dir.path());
    let sw = orch(&["w1:p2", "w1:p3", "w1:p4"]);
    let sv = m
        .reconcile(w.snap(), &sw, Some(&home()), &set(&[]), true, false)
        .await
        .unwrap();
    for (tab, ag) in [("w1:t2", "w1:p2"), ("w1:t3", "w1:p3"), ("w1:t4", "w1:p4")] {
        let order = w.order(tab);
        assert_eq!(order.len(), 2);
        assert_eq!(order[1], ag);
        assert!(
            w.s().argv.contains_key(&order[0]),
            "the panel is on the left"
        );
        assert!(
            w.calls().contains(&format!("swap {ag} {}", order[0])),
            "the agent is the swap's source"
        );
    }
    assert_eq!(w.s().focus, ("w1:t1".into(), "w1:p1".into()));
    let calls = w.calls();
    assert_eq!(
        calls[calls.len() - 2..],
        ["focus_tab w1:t1".to_string(), "focus_pane w1:p1".into()]
    );
    let map = m.panels_map(&sv, &set(&[]));
    for tab in ["w1:t2", "w1:t3", "w1:t4"] {
        assert_eq!(map[tab], w.order(tab)[0]);
    }
    assert_eq!(map.len(), 3);
    w.s().calls.clear();
    pass(&mut m, &w, &sw, &[], false).await; // nothing missing: nothing done
    assert!(w.calls().is_empty());
}

#[tokio::test]
async fn an_operator_who_moves_during_the_batch_is_left_there() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    w.s().tabs.push(("w2:t9".into(), vec![pane("w2:p9")]));
    w.s().on_swap = Some(Box::new(|s: &mut State| {
        if s.calls.iter().filter(|c| c.starts_with("swap")).count() == 2 {
            s.record(Focus::Pane("w2:p9".into())); // a pane the batch never touches
        }
    }));
    let mut m = manager(&w, dir.path());
    pass(&mut m, &w, &orch(&["w1:p2", "w1:p3", "w1:p4"]), &[], true).await;
    assert!(!w.calls().iter().any(|c| c.starts_with("focus")));
}

#[tokio::test]
async fn the_restore_skips_only_the_batchs_own_events_in_order() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    w.s().on_swap = Some(Box::new(|s: &mut State| {
        if s.calls.iter().filter(|c| c.starts_with("swap")).count() == 2 {
            s.record(Focus::Tab("w1:t2".into())); // after t3's swap: back to t2, by hand
            s.record(Focus::Pane("w1:p2".into()));
        }
    }));
    let mut m = manager(&w, dir.path());
    pass(&mut m, &w, &orch(&["w1:p2", "w1:p3", "w1:p4"]), &[], true).await;
    assert!(!w.calls().iter().any(|c| c.starts_with("focus")));
}

#[test]
fn the_batchs_own_events_may_skip_one_but_never_go_back() {
    let expected = [t("a"), p("1"), t("b"), p("2")];
    assert_eq!(
        operator_moved(&[t("a"), p("1"), t("b"), p("2")], &expected),
        None
    );
    assert_eq!(
        operator_moved(&[p("1"), p("2")], &expected),
        None,
        "herdr may leave a tab event out"
    );
    assert_eq!(
        operator_moved(&[t("a"), p("1"), t("a")], &expected),
        Some("a".into()),
        "back is the operator's"
    );
    assert_eq!(operator_moved(&[t("x")], &expected), Some("x".into()));
    assert_eq!(
        operator_moved(&[p(""), t("a")], &expected),
        None,
        "an empty id is no event"
    );
    assert_eq!(
        operator_moved(&[t("a"), p("2"), p("1")], &expected),
        Some("1".into())
    );
}

#[tokio::test]
async fn after_the_start_only_the_operators_tab_gets_a_panel() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let mut m = manager(&w, dir.path());
    pass(&mut m, &w, &orch(&["w1:p2"]), &[], true).await;
    w.s().calls.clear();
    let sw = orch(&["w1:p2", "w1:p3"]); // a new orchestrator in t3; the operator is elsewhere
    pass(&mut m, &w, &sw, &[], false).await;
    assert!(w.calls().is_empty());
    w.s().focus = ("w1:t3".into(), "w1:p3".into()); // they enter its tab, on the agent
    pass(&mut m, &w, &sw, &[], false).await;
    let panel = w.order("w1:t3")[0].clone();
    assert_eq!(
        w.calls(),
        ["open w1:p3".to_string(), format!("swap w1:p3 {panel}")]
    );
    assert_eq!(w.s().focus, ("w1:t3".into(), "w1:p3".into()));
}

#[tokio::test]
async fn a_lazy_panel_on_another_pane_of_the_tab_gives_that_pane_the_focus_back() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    w.s().tab("w1:t2").push(pane("w1:p5"));
    w.s().focus = ("w1:t2".into(), "w1:p5".into());
    let mut m = manager(&w, dir.path());
    m.started = true;
    pass(&mut m, &w, &orch(&["w1:p2"]), &[], false).await;
    assert_eq!(w.order("w1:t2")[1], "w1:p2");
    assert_eq!(w.calls().last().unwrap(), "focus_pane w1:p5");
    assert_eq!(w.s().focus, ("w1:t2".into(), "w1:p5".into()));
}

#[tokio::test]
async fn a_lazy_panel_waits_on_the_right_when_the_operator_left_before_the_swap() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let mut m = manager(&w, dir.path());
    m.started = true;
    w.s().focus = ("w1:t2".into(), "w1:p2".into());
    w.s().on_open = Some(Box::new(|s: &mut State| {
        s.focus = ("w1:t1".into(), "w1:p1".into())
    }));
    let sw = orch(&["w1:p2"]);
    pass(&mut m, &w, &sw, &[], false).await;
    assert_eq!(w.order("w1:t2")[0], "w1:p2", "right of the agent");
    assert!(!w.calls().iter().any(|c| c.starts_with("swap")), "no pull");
    assert!(m.right.contains("w1:t2"));
    w.s().on_open = None;
    w.s().focus = ("w1:t2".into(), "w1:p2".into()); // the next visit swaps it left
    pass(&mut m, &w, &sw, &[], false).await;
    let order = w.order("w1:t2");
    assert_eq!(order[1], "w1:p2");
    assert!(w.calls().contains(&format!("swap w1:p2 {}", order[0])));
    assert!(m.right.is_empty());
}

#[tokio::test]
async fn a_tab_herdr_could_not_check_gets_no_panel() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    w.s().tab("w1:t2").push(pane("w1:p60"));
    w.s().runs = Some(Box::new(
        |_, p| if p == "w1:p60" { None } else { Some(false) },
    ));
    let mut m = manager(&w, dir.path());
    pass(&mut m, &w, &orch(&["w1:p2"]), &[], true).await;
    assert!(w.calls().is_empty());
}

#[tokio::test]
async fn a_known_panel_is_not_asked_about_on_every_pass() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    w.s().tab("w1:t2").insert(0, pane("w1:p50"));
    w.s().viewer("w1:p50");
    let mut m = manager(&w, dir.path());
    for _ in 0..3 {
        let start = !m.started;
        pass(&mut m, &w, &orch(&["w1:p2"]), &[], start).await;
    }
    assert_eq!(w.s().asked.iter().filter(|p| *p == "w1:p50").count(), 1);
}

#[tokio::test]
async fn a_not_a_panel_answer_is_believed_for_a_while_then_asked_again() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    w.s().tab("w1:t2").insert(0, pane("w1:p50"));
    let now = std::sync::Arc::new(std::sync::Mutex::new(std::time::Instant::now()));
    let c = now.clone();
    let mut m = manager_with(
        &w,
        dir.path(),
        std::sync::Arc::new(move || *c.lock().unwrap()),
    );
    m.started = true;
    let sw = orch(&["w1:p2"]);
    for _ in 0..2 {
        pass(&mut m, &w, &sw, &[], false).await;
    }
    let asked = || w.s().asked.iter().filter(|p| *p == "w1:p50").count();
    assert_eq!(asked(), 1);
    *now.lock().unwrap() += crate::panels::NOT_PANEL;
    pass(&mut m, &w, &sw, &[], false).await;
    assert_eq!(asked(), 2, "NOT_PANEL is over");
}

#[tokio::test]
async fn single_follows_only_tabs_where_the_orchestrator_is_alone() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    w.s().tab("w1:t3").push(pane("w1:p5"));
    let mut m = manager(&w, dir.path());
    let sv = m
        .reconcile(
            w.snap(),
            &orch(&["w1:p2", "w1:p3"]),
            Some(&home()),
            &set(&[]),
            true,
            true,
        )
        .await
        .unwrap();
    assert!(sv.targets.contains_key("w1:t2") && !sv.targets.contains_key("w1:t3"));
    assert_eq!(w.order("w1:t3"), ["w1:p3", "w1:p5"]);
    // its own panel does not crowd a tab
    let sv = m
        .reconcile(
            w.snap(),
            &orch(&["w1:p2", "w1:p3"]),
            Some(&home()),
            &set(&[]),
            false,
            true,
        )
        .await
        .unwrap();
    assert!(sv.targets.contains_key("w1:t2"));
}

#[tokio::test]
async fn only_local_claude_orchestrators_outside_home_in_its_workspace_are_targets() {
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    w.s().tab("w1:t1").push(agent("w1:p9")); // in the home tab
    w.s().tab("w1:t3")[0].agent = Some("codex".into());
    let mut m = manager(&w, dir.path());
    let mut sw = orch(&["w1:p2", "w1:p3", "w1:p4", "w1:p9", "w1:p77"]);
    sw.orchestrators[2].machine = Some("box".into());
    let mut snap = w.snap();
    snap.panes
        .iter_mut()
        .find(|p| p.pane_id == "w1:p2")
        .unwrap()
        .workspace_id = Some("w2".into());
    let sv = m
        .reconcile(snap.clone(), &sw, Some(&home()), &set(&[]), false, false)
        .await
        .unwrap();
    assert!(sv.targets.is_empty(), "{:?}", sv.targets);
    // without a home: any workspace, any tab
    let sv = m
        .reconcile(snap, &sw, None, &set(&[]), false, false)
        .await
        .unwrap();
    assert_eq!(sv.targets.keys().collect::<Vec<_>>(), ["w1:t1", "w1:t2"]);
}

#[tokio::test]
async fn a_panel_already_left_of_its_agent_is_not_swapped_again() {
    // without the guard every pass in the operator's tab would swap and pull the focus
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    let mut m = manager(&w, dir.path());
    pass(&mut m, &w, &orch(&["w1:p2"]), &[], true).await;
    let panel = w.order("w1:t2")[0].clone();
    for focus in ["w1:p2", panel.as_str()] {
        w.s().focus = ("w1:t2".into(), focus.into());
        w.s().calls.clear();
        pass(&mut m, &w, &orch(&["w1:p2"]), &[], false).await;
        assert!(w.calls().is_empty(), "{:?}", w.calls());
        assert_eq!(w.s().focus.1, focus);
    }
}

#[tokio::test]
async fn the_home_pane_is_never_counted_as_a_panel() {
    // panorama's own pane beside an agent (an older version left it there) is no panel, even
    // if herdr's process answer were to say so
    let (w, dir) = (World::new(), tempfile::tempdir().unwrap());
    w.s().tab("w1:t2").insert(0, pane("w1:p5"));
    w.s().runs = Some(Box::new(|_, _| Some(true)));
    let mut m = manager(&w, dir.path());
    let me = crate::panels::HomeRef {
        pane_id: "w1:p5".into(),
        tab_id: "w1:t1".into(),
    };
    let sv = m
        .reconcile(
            w.snap(),
            &orch(&["w1:p2"]),
            Some(&me),
            &set(&[]),
            true,
            false,
        )
        .await
        .unwrap();
    assert!(!w.s().asked.contains(&"w1:p5".to_string()));
    assert!(sv.panels["w1:t2"].iter().all(|p| p != "w1:p5"));
    assert!(w.calls().contains(&"open w1:p2".to_string()));
}

#[test]
fn the_settle_wait_keeps_its_margin() {
    // test_follow.py::test_r16_the_settle_wait_keeps_its_margin: herdr delivers focus events
    // up to ~100 ms late (measured)
    assert!(crate::panels::SYNC >= std::time::Duration::from_millis(300));
}
