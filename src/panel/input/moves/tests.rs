use crate::panel::input::Key;
use crate::panel::input::fixtures::panel;

#[test]
fn arrows_move_among_the_shown_rows_and_resume_from_a_parked_cursor() {
    // a tab without an orchestrator: the narrow layout, arrows only move
    let (mut p, t) = panel("w1:t9", "w1:t1");
    assert_eq!(p.view.orch.as_deref(), Some("w1:p1"));
    p.on_key(Key::Move(1), t);
    assert_eq!(p.view.orch.as_deref(), Some("w1:p2"));
    p.on_key(Key::Move(-1), t);
    p.on_key(Key::Move(-1), t); // the top stays the top
    assert_eq!(p.view.orch.as_deref(), Some("w1:p1"));
    // parked off w1:p3 (the filter hid it): ↓ resumes after it, ↑ before it
    p.view.orch = None;
    p.parked = Some("w1:p3".into());
    p.on_key(Key::Move(1), t);
    assert_eq!(p.view.orch.as_deref(), Some("w1:p4"));
    p.view.orch = None; // still parked off w1:p3: ↑ goes to the row before it
    p.on_key(Key::Move(-1), t);
    assert_eq!(p.view.orch.as_deref(), Some("w1:p2"));
}

#[test]
fn right_and_left_walk_the_columns_in_the_narrow_layout_only() {
    let (mut p, t) = panel("w1:t9", "w1:t1");
    p.on_key(Key::Right, t);
    assert_eq!(p.view.focus, 1);
    p.on_key(Key::Right, t);
    assert_eq!(p.view.focus, 2);
    p.on_key(Key::Left, t);
    assert_eq!(p.view.focus, 1);
    // docked: one column
    let (mut d, t) = panel("w1:t2", "w1:t2");
    d.on_key(Key::Right, t);
    assert_eq!(d.view.focus, 0);
}

#[test]
fn a_point_selects_an_identity_and_focuses_its_column() {
    let (mut p, _) = panel("w1:t9", "w1:t1");
    // the first thread and agent the nav shows under w1:p2
    let v = panorama_core::view::View {
        orch: Some("w1:p2".into()),
        ..Default::default()
    };
    let thread = panorama_core::view::nav::nav(&p.swarm, &v)[1][0].clone();
    let v = panorama_core::view::View {
        thread: Some(thread.clone()),
        ..v
    };
    let agent = panorama_core::view::nav::nav(&p.swarm, &v)[2][0].clone();
    assert!(p.shown("w1:p2", Some(&thread), Some(&agent)));
    p.point("w1:p2", Some(&thread), Some(&agent));
    assert_eq!(
        (
            p.view.orch.as_deref(),
            p.view.thread.as_deref(),
            p.view.focus
        ),
        (Some("w1:p2"), Some(thread.as_str()), 2)
    );
    assert!(!p.shown("w1:p9", None, None));
}
