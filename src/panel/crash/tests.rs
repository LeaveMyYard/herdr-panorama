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
