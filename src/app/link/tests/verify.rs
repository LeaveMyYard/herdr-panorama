//! The core's pane check: only `panorama panel` running as the connecting process itself, in
//! the pane it names; any doubt is no.

use crate::app::link::verify::{HerdrVerify, runs_as};
use crate::core::Verify;
use herdr_agent_kit::herdr_api::ProcessInfo;
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use serde_json::json;

fn info(procs: serde_json::Value) -> ProcessInfo {
    serde_json::from_value(
        json!({"pane_id": "w1:p9", "shell_pid": 50, "foreground_processes": procs}),
    )
    .unwrap()
}

#[test]
fn only_our_viewer_as_the_peer_itself_counts() {
    let panel = |pid: i64, argv: &[&str]| json!([{"pid": pid, "name": "x", "argv": argv}]);
    let ours = ["/p/.venv/bin/python", "/p/.venv/bin/panorama", "panel"];
    assert!(runs_as(&info(panel(77, &ours)), "panel", 77));
    assert!(runs_as(
        &info(panel(77, &["/p/target/release/panorama", "panel"])),
        "panel",
        77
    ));
    assert!(
        !runs_as(&info(panel(78, &ours)), "panel", 77),
        "another process in that pane"
    );
    assert!(
        !runs_as(
            &info(panel(77, &["/p/.venv/bin/panorama", "tui"])),
            "panel",
            77
        ),
        "the home view"
    );
    // any `panorama panel` pair in the argv counts, as Python's `runs_panorama`
    assert!(runs_as(
        &info(panel(77, &["vim", "panorama", "panel"])),
        "panel",
        77
    ));
    assert!(!runs_as(&info(panel(77, &["zsh"])), "panel", 77));
    assert!(!runs_as(&info(json!([])), "panel", 77));
    assert!(!runs_as(&info(serde_json::Value::Null), "panel", 77));
    let no_pid = json!([{"name": "x", "argv": ours}]);
    assert!(!runs_as(&info(no_pid), "panel", 77));
}

#[tokio::test(flavor = "current_thread")]
async fn herdrs_answer_decides_and_an_error_is_no() {
    let fake = FakeHerdr::start();
    let v = HerdrVerify(fake.client());
    fake.on("pane.process_info", Reply::Result(json!({"type": "pane_process_info", "process_info": {"pane_id": "w1:p9",
        "shell_pid": 77, "foreground_processes": [{"pid": 77, "name": "panorama", "argv": ["/p/panorama", "panel"]}]}})));
    assert!(v.verify("w1:p9", 77).await);
    assert_eq!(
        fake.params_of("pane.process_info")[0]["pane_id"],
        "w1:p9",
        "the pane it claims"
    );
    assert!(!v.verify("w1:p9", 78).await);
    fake.on(
        "pane.process_info",
        Reply::error("pane_not_found", "pane not found"),
    );
    assert!(!v.verify("w1:p9", 77).await, "herdr cannot say: unverified");
}

#[tokio::test(flavor = "current_thread")]
async fn a_hello_from_the_home_views_own_process_is_not_trusted() {
    // the pid runs `panorama tui` in that pane: the home view, never a viewer
    let fake = FakeHerdr::start();
    fake.on("pane.process_info", Reply::Result(json!({"type": "pane_process_info", "process_info": {"pane_id": "w1:p1",
        "shell_pid": 77, "foreground_processes": [{"pid": 77, "name": "panorama", "argv": ["/p/.venv/bin/panorama", "tui"]}]}})));
    assert!(!HerdrVerify(fake.client()).verify("w1:p1", 77).await);
}
