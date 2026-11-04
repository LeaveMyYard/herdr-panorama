//! The changed Rust binary against only synthetic herdr and Codex processes.

mod codex_support;

use codex_support::{Fixture, ROOT_A, snapshot};
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use serde_json::{Value, json};
use std::process::Command;
use std::time::{Duration, Instant};

fn run(fixture: &Fixture, fake: &FakeHerdr) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_panorama"))
        .env_clear()
        .args(["snapshot", "--json"])
        .env("PATH", fixture.executable_path())
        .env("HOME", fixture.root())
        .env("CODEX_HOME", fixture.root())
        .env("CLAUDE_CONFIG_DIR", fixture.root().join("claude"))
        .env("HERDR_SOCKET_PATH", fake.socket())
        .env("HERDR_BIN_PATH", "/nonexistent/herdr")
        .env("HERDR_PLUGIN_CONFIG_DIR", fixture.root().join("config"))
        .env("HERDR_INBOX_DB", fixture.root().join("missing.db"))
        .env("PANORAMA_EVENTS_DIR", fixture.root().join("events"))
        .env("PANORAMA_HOOKS_DIR", fixture.root().join("hooks"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn one_shot_snapshot_warms_history_once_and_preserves_exact_mixed_identity() {
    let fixture = Fixture::new("paginated");
    let fake = FakeHerdr::start();
    fake.on(
        "session.snapshot",
        Reply::Result(json!({"snapshot":snapshot()})),
    );
    let model = run(&fixture, &fake);
    assert_eq!(model["orchestrators"].as_array().unwrap().len(), 4);
    assert_eq!(model["orchestrators"][0]["harness"], "claude");
    let codex = &model["orchestrators"][1];
    assert_eq!(codex["harness"], "codex");
    assert_eq!(codex["session_id"], ROOT_A);
    assert!(codex["last_activity"].is_number());
    // the root's child and grandchild; the root thread itself is the root, no agent
    assert_eq!(codex["threads"][0]["id"], panorama_core::model::TREE_THREAD);
    assert_eq!(codex["threads"][0]["agents"].as_array().unwrap().len(), 2);
    let calls = fake.calls();
    assert_eq!(
        calls
            .iter()
            .filter(|(method, _)| method == "session.snapshot")
            .count(),
        1
    );
    assert!(
        calls
            .iter()
            .all(|(method, _)| matches!(method.as_str(), "session.snapshot" | "pane.process_info")),
        "{calls:?}"
    );
}

#[test]
fn timeout_keeps_panes_and_reaps_the_owned_app_server_before_cli_exit() {
    let fixture = Fixture::new("hang");
    let fake = FakeHerdr::start();
    fake.on(
        "session.snapshot",
        Reply::Result(json!({"snapshot":snapshot()})),
    );
    let start = Instant::now();
    let model = run(&fixture, &fake);
    // The production budget is 750ms; allow scheduling overhead while rejecting
    // the adapter's 4s read timeout and the fixture's 30s stalled response.
    assert!(start.elapsed() < Duration::from_secs(3));
    assert_eq!(model["orchestrators"].as_array().unwrap().len(), 4);
    assert!(
        model["orchestrators"]
            .as_array()
            .unwrap()
            .iter()
            .all(|orchestrator| orchestrator["last_activity"].is_null()
                && orchestrator["threads"][0]["agents"]
                    .as_array()
                    .unwrap()
                    .is_empty())
    );
    let pid = std::fs::read_to_string(fixture.root().join("pid")).unwrap();
    #[cfg(target_os = "linux")]
    assert!(
        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "owned child {pid} outlived snapshot"
    );
    #[cfg(not(target_os = "linux"))]
    {
        let output = Command::new("ps")
            .args(["-p", pid.trim(), "-o", "pid="])
            .output()
            .unwrap();
        assert!(
            String::from_utf8_lossy(&output.stdout).trim().is_empty(),
            "owned child {pid} outlived snapshot"
        );
    }
}
