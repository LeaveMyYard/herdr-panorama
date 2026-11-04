use super::*;

#[test]
fn a_refused_hello_says_why_in_the_launch_log() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("state");
    note_refused(&state, "protocol", "2026-11-04 10:00:00 +0000");
    let log = std::fs::read_to_string(state.join("panel-launch.log")).unwrap();
    assert_eq!(
        log,
        format!(
            "2026-11-04 10:00:00 +0000 panorama: the core refused this panel (protocol; it \
             speaks protocol {}): restart the panorama home view after an upgrade\n",
            crate::proto::PROTOCOL
        )
    );
}
