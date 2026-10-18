use super::*;

#[test]
fn a_crash_leaves_the_marker_and_the_errors_end() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("state");
    let error = format!("{}é{}", "x".repeat(LOG_BYTES), "the end");
    mark(&state, Some("w1:p9"), &error, "2026-10-18 10:00:00 +0000");
    let at: f64 = fs::read_to_string(marker(&state, "w1:p9"))
        .unwrap()
        .parse()
        .unwrap();
    assert!(at > 1.7e9);
    let log = fs::read_to_string(state.join("panel-crash-w1:p9.log")).unwrap();
    let (stamp, body) = log.split_once('\n').unwrap();
    assert_eq!(stamp, "2026-10-18 10:00:00 +0000");
    assert!(body.ends_with("the end") && body.len() <= LOG_BYTES && !body.starts_with('\u{fffd}'));
}

#[test]
fn without_a_pane_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    mark(dir.path(), None, "boom", "now");
    mark(dir.path(), Some(""), "boom", "now");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn the_hook_marks_a_crash_but_leaves_a_tasks_panic_to_the_task() {
    use crate::panel::actor::guard::{block_on, guarded, hook_lock};
    let _hook = hook_lock();
    // #77 L3: a herdr-side task's panic is its own failure, not the panel's crash
    let dir = tempfile::tempdir().unwrap();
    let previous = std::panic::take_hook();
    std::panic::set_hook(hook(
        dir.path().into(),
        Some("w9:p9".into()),
        || {},
        Box::new(|_| {}),
    ));
    let task = block_on(guarded(async { panic!("in a task") }));
    let task_marked = marker(dir.path(), "w9:p9").exists();
    let crash = std::panic::catch_unwind(|| panic!("in the panel"));
    std::panic::set_hook(previous);
    assert!(task.is_err() && crash.is_err());
    assert!(!task_marked, "a task's panic marked a crash");
    assert!(
        marker(dir.path(), "w9:p9").exists(),
        "the panel's panic left no marker"
    );
}
