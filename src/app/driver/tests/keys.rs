//! tests/test_tui.py's keys: the cursor across the columns, the filter, folding, what the
//! action keys act on, and the home view versus a popup.

use super::*;
use panorama_core::view::Filter;
use panorama_core::view::nav::nav;
use panorama_core::view::state::{DONE_CARD, FIN_CARD, NEW_ROW};

const WIDE: (i32, i32) = (160, 45);

fn keys(r: &mut Run, ks: &[Key], now: Instant) -> Vec<Effect> {
    ks.iter().flat_map(|k| r.key(*k, now)).collect()
}

#[test]
fn arrow_navigation_across_columns() {
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.t0; // moving is never armed
    assert_eq!(
        r.sel(),
        (0, Some("w1:p1"), Some("ref:#42"), Some("a1c3d90"))
    );
    keys(&mut r, &[Key::Move(1)], now);
    assert_eq!(
        r.sel(),
        (0, Some("w1:p2"), Some("wf:wf_search"), Some("j1a2b3c"))
    );
    keys(&mut r, &[Key::Move(-1), Key::Right, Key::Move(1)], now);
    assert_eq!(
        r.sel(),
        (1, Some("w1:p1"), Some("ref:#44"), Some("b3a9c41"))
    );
    keys(&mut r, &[Key::Right, Key::Move(1)], now);
    let fin = format!("{FIN_CARD}ref:#44");
    assert_eq!(
        r.sel(),
        (2, Some("w1:p1"), Some("ref:#44"), Some(fin.as_str()))
    );
    keys(&mut r, &[Key::Left, Key::Move(-1)], now); // back to #42: its remembered agent
    assert_eq!(
        r.sel(),
        (1, Some("w1:p1"), Some("ref:#42"), Some("a1c3d90"))
    );
}

#[test]
fn the_filter_cycles_and_reselects() {
    let mut r = Run::demo("w1:p2", WIDE);
    let now = r.settled();
    r.key(Key::Filter, now);
    assert_eq!(
        (r.d.app.view.filter, r.d.app.view.orch.as_deref()),
        (Filter::Running, Some("w1:p2"))
    );
    r.key(Key::Filter, now); // needs me: Search refactor needs nothing -> the first shown
    assert_eq!(
        (r.d.app.view.filter, r.d.app.view.orch.as_deref()),
        (Filter::Needs, Some("w1:p1"))
    );
    assert_eq!(
        nav(&r.d.app.swarm, &r.d.app.view)[0],
        ["w1:p1", "w1:p3", NEW_ROW]
    );
    r.key(Key::Filter, now);
    assert_eq!(r.d.app.view.filter, Filter::All);
}

#[test]
fn enter_unfolds_the_complete_card() {
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    keys(
        &mut r,
        &[Key::Right, Key::Move(1), Key::Move(1), Key::Move(1)],
        now,
    );
    assert_eq!(
        r.d.app.view.thread.as_deref(),
        Some(format!("{DONE_CARD}w1:p1").as_str())
    );
    r.key(Key::Enter, now);
    assert!(r.d.app.view.done_open.contains("w1:p1"));
    r.key(Key::Move(1), now);
    assert_eq!(r.d.app.view.thread.as_deref(), Some("ref:#40"));
}

#[test]
fn answer_on_a_blocked_session_goes_to_the_pane() {
    let mut r = Run::demo("w1:p3", WIDE);
    assert_eq!(
        r.sel(),
        (0, Some("w1:p3"), Some("main:w1:p3"), Some("main:w1:p3"))
    );
    assert!(r.text().contains("! Publish the v2 docs now or after the"));
    let now = r.settled();
    let out = r.key(Key::Answer, now);
    assert!(goes_to(&out, "w1:p3"));
    assert!(!out.iter().any(|e| matches!(e, Effect::OpenInbox { .. })));
}

#[test]
fn the_answer_key_opens_the_asking_agents_question_group() {
    // test_tui.py::test_answer_opens_the_inbox_after_the_popup_closes: the key's half (the
    // actor opens it once the popup has closed, P4 8c)
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    keys(
        &mut r,
        &[Key::Right, Key::Move(1), Key::Move(1), Key::Right],
        now,
    );
    assert_eq!(r.d.app.view.agent.as_deref(), Some("c1d2e3f"));
    let out = r.key(Key::Answer, now);
    assert!(
        matches!(
            out.as_slice(),
            [Effect::OpenInbox {
                question: Some(_),
                pane: None
            }]
        ),
        "{out:?}"
    );
}

#[test]
fn no_inbox_means_no_inbox_actions() {
    let (mut sw, now_s) = wire("demo");
    sw.inbox_ok = false;
    let mut r = Run::new((sw, now_s), "w1:p1", false, WIDE);
    assert!(!r.text().contains("i inbox"));
    let now = r.settled();
    let mut out = r.key(Key::Inbox, now);
    out.extend(keys(
        &mut r,
        &[
            Key::Right,
            Key::Move(1),
            Key::Move(1),
            Key::Right,
            Key::Answer,
        ],
        now,
    ));
    assert!(
        !out.iter()
            .any(|e| matches!(e, Effect::OpenInbox { .. } | Effect::Quit)),
        "{out:?}"
    );
}

#[test]
fn parked_cursor_keys_act_on_nothing() {
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    keys(&mut r, &[Key::Right, Key::Right], now);
    let gone = r.d.app.view.agent.clone().unwrap();
    let mut sw = wire("demo").0;
    sw.orchestrators[0].threads[0]
        .agents
        .retain(|a| a.id != gone);
    r.refresh(sw, now);
    assert_eq!(r.d.app.view.agent, None);
    r.d.app.armed_at = r.t0;
    let out = keys(&mut r, &[Key::Goto, Key::Answer], now);
    assert!(out.is_empty(), "{out:?}");
}

#[test]
fn a_popup_goes_to_the_pane_and_esc_or_q_closes_it() {
    // test_tui.py::test_goto_focuses_the_orchestrator_pane_and_closes: the key's half (the
    // actor focuses the pane and then closes the popup, P4 8c)
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    keys(&mut r, &[Key::Right, Key::Right], now);
    assert!(goes_to(&r.key(Key::Goto, now), "w1:p1"));
    // in a popup Enter on an orchestrator opens its threads; Esc and q close it
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    assert!(r.key(Key::Enter, now).is_empty() && r.sel().0 == 1);
    assert_eq!(r.key(Key::Escape, now), [Effect::Quit]);
    assert_eq!(r.key(Key::Quit, now), [Effect::Quit]);
    assert!(r.key(Key::Zoom, now).is_empty()); // a popup does not zoom
}

#[test]
fn the_home_view_stays_open() {
    // test_tui.py::test_home_mode_keeps_the_view_open: Enter on an orchestrator goes to its
    // pane, a stray Esc does nothing, g goes to the pane; z zooms
    let mut r = Run::new(wire("demo"), "w1:p1", true, WIDE);
    let now = r.settled();
    let mut out = r.key(Key::Enter, now);
    assert!(r.key(Key::Escape, now).is_empty());
    keys(&mut r, &[Key::Right, Key::Right], now);
    out.extend(r.key(Key::Goto, now));
    let gotos = out
        .iter()
        .filter(|e| matches!(e, Effect::Goto { pane, .. } if pane == "w1:p1"))
        .count();
    assert_eq!(gotos, 2);
    assert!(!out.contains(&Effect::Quit));
    assert_eq!(r.key(Key::Zoom, now), [Effect::Zoom]);
}

#[test]
fn the_new_row_is_last_stable_and_opens_new_agent() {
    // test_tui.py::test_new_row_is_last_stable_and_opens_the_form: the key's half (the form
    // is design 1C, P4 PR 9)
    let mut r = Run::new(wire("demo"), "w1:p1", true, WIDE);
    let now = r.settled();
    keys(&mut r, &[Key::Move(1); 4], now);
    assert_eq!(
        (r.d.app.view.orch.as_deref(), r.d.app.view.thread.as_deref()),
        (Some(NEW_ROW), None)
    );
    assert!(r.text().contains("+  New agent"));
    r.refresh(wire("demo").0, now); // a refresh never moves or changes the + row
    assert_eq!(r.d.app.view.orch.as_deref(), Some(NEW_ROW));
    assert!(r.d.app.armed(now));
    let out = r.key(Key::Enter, now);
    // no orchestrator here has a directory: the actor gives the popup the home directory
    assert_eq!(out, [Effect::NewAgent { cwd: None }]);
    assert!(keys(&mut r, &[Key::Goto, Key::Answer], now).is_empty()); // nothing to act on
}

#[test]
fn help_says_the_keys() {
    let mut r = Run::demo("w1:p1", WIDE);
    let out = r.key(Key::Help, r.t0);
    assert!(matches!(out.as_slice(), [Effect::Toast(t)] if t.title.as_deref() == Some("keys")));
}

#[test]
fn g_on_another_machines_orchestrator_asks_for_its_own_server() {
    // Python `_goto_remote`: never a local focus; its agents are seen
    let mut r = Run::new(wire("remote"), "abc123:w1:p1", false, WIDE);
    let now = r.settled();
    let out = r.key(Key::Goto, now);
    assert!(
        matches!(out.last(), Some(Effect::GotoRemote { machine_id, machine, pane, .. })
        if machine_id == "abc123" && machine == "devbox" && pane == "abc123:w1:p1"),
        "{out:?}"
    );
    assert!(!goes_to(&out, "abc123:w1:p1"));
}

#[test]
fn g_waits_for_the_arming_and_marks_the_orchestrators_agents_seen() {
    use panorama_core::view::Unseen;
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    r.d.app.armed_at = now + Duration::from_secs(30); // right after a background change
    assert!(r.key(Key::Goto, now).is_empty());
    r.d.app.armed_at = r.t0;
    r.d.app.view.unseen.insert("a1c3d90".into(), Unseen::Done);
    let out = r.key(Key::Goto, now);
    let seen = out.iter().find_map(|e| match e {
        Effect::Seen(ids) => Some(ids.clone()),
        _ => None,
    });
    assert!(
        seen.is_some_and(|ids| ids.contains(&"a1c3d90".to_owned())),
        "{out:?}"
    );
    assert!(!r.d.app.view.unseen.contains_key("a1c3d90"));
    assert!(goes_to(&out, "w1:p1"));
}

#[test]
fn a_click_on_a_notice_dismisses_it_and_goes_no_further() {
    use crate::ui::input::Toast;
    let mut r = Run::demo("w1:p1", WIDE);
    let now = r.settled();
    r.d.toasts
        .push(Toast::new(Some("keys"), "a notice", 10), now);
    r.paint(Painted::Other, now);
    let before = format!("{:?}", r.sel());
    let (x, y) = (WIDE.0 - 10, WIDE.1 - 4); // inside the notice, bottom right
    let (out, cause) = r.d.pointer(Pointer::Click, x, y, now);
    assert!(out.is_empty() && cause == Some(Painted::Other));
    assert_eq!(r.d.toasts.next_expiry(), None); // dismissed
    assert_eq!(format!("{:?}", r.sel()), before);
}
