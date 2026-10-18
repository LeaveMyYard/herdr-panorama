use crate::panel::input::fixtures::panel;
use crate::panel::input::{Effect, Key};
use std::time::Duration;

#[test]
fn enter_on_another_orchestrator_in_the_docked_list_goes_into_it() {
    // docked in tab 2, herdr shows tab 2; the cursor moves to tab 4 (Billing Migrations)
    let (mut p, t) = panel("w1:t2", "w1:t2");
    p.view.orch = Some("w1:p4".into());
    let out = p.on_key(Key::Enter, t);
    // with the arrows switching the chats, Enter is the way into the agent: no keep_focus
    assert!(
        matches!(&out[..], [Effect::Goto { pane, keep_focus: false, seen, .. }] if pane == "w1:p4" && !seen.is_empty())
    );
    // dock_on_arrows off: Enter switches and keeps the keyboard on the panels
    p.input.settings.arrows = false;
    let out = p.on_key(Key::Enter, t);
    assert!(matches!(
        &out[..],
        [Effect::Goto {
            keep_focus: true,
            ..
        }]
    ));
}

#[test]
fn nothing_acts_before_the_keys_are_armed_or_without_a_core() {
    let (mut p, t) = panel("w1:t2", "w1:t2");
    assert!(
        p.on_key(Key::Enter, t - Duration::from_millis(700))
            .is_empty()
    );
    p.on_connected(false);
    let out = p.on_key(Key::Goto, t);
    assert!(matches!(&out[..], [Effect::Toast(toast)] if toast.text == "panorama is restarting…"));
}

#[test]
fn answer_opens_the_inbox_or_the_blocked_pane() {
    let (mut p, t) = panel("w1:t2", "w1:t2");
    // Payments UI (tab 1) has a question: its oldest group
    p.view.orch = Some("w1:p1".into());
    assert!(
        matches!(&p.on_key(Key::Answer, t)[..], [Effect::OpenInbox { question: None, pane: Some(pane) }] if pane == "w1:p1")
    );
    // Data Pipeline (tab 2) is blocked: answered in its pane
    p.view.orch = Some("w1:p2".into());
    assert!(
        matches!(&p.on_key(Key::Answer, t)[..], [Effect::Goto { pane, .. }] if pane == "w1:p2")
    );
    assert_eq!(
        p.on_key(Key::Inbox, t),
        [Effect::OpenInbox {
            question: None,
            pane: None
        }]
    );
}

#[test]
fn f_cycles_the_filter_and_keeps_it_against_the_cores_older_word() {
    let (mut p, t) = panel("w1:t2", "w1:t2");
    assert_eq!(
        p.on_key(Key::Filter, t),
        [Effect::SendFilter(panorama_core::view::Filter::Running)]
    );
    // the core still says "all": ignored for a second
    p.on_state(
        super::super::fixtures::state(serde_json::json!({"filter": "all"})),
        t + Duration::from_millis(500),
    );
    assert_eq!(p.view.filter, panorama_core::view::Filter::Running);
    p.on_state(
        super::super::fixtures::state(serde_json::json!({"filter": "all"})),
        t + Duration::from_secs(2),
    );
    assert_eq!(p.view.filter, panorama_core::view::Filter::All);
}

#[test]
fn notices_and_quitting() {
    let (mut p, t) = panel("w1:t2", "w1:t2");
    assert!(
        matches!(&p.on_key(Key::Help, t)[..], [Effect::Toast(toast)] if toast.title.as_deref() == Some("keys"))
    );
    assert!(
        matches!(&p.on_key(Key::Transcript, t)[..], [Effect::Toast(toast)] if toast.title.as_deref() == Some("transcript"))
    );
    assert_eq!(p.on_key(Key::Quit, t), [Effect::Quit]);
    assert!(p.on_key(Key::Escape, t).is_empty());
    assert!(matches!(
        &p.on_key(Key::NewAgent, t)[..],
        [Effect::NewAgent { .. }]
    ));
}
