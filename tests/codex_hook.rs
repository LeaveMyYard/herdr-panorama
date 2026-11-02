//! Hook telemetry cannot become model instructions or block a session on failure.
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
fn command(dir: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_panorama"));
    command
        .arg("codex-hook")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}
#[test]
fn invalid_ignored_or_unavailable_hook_always_returns_empty_object() {
    let dir = tempfile::tempdir().unwrap();
    for input in [b"not-json".to_vec(),vec![b' ';65537],br#"{"hook_event_name":"SubagentStart","session_id":"11111111-1111-4111-8111-111111111111","source":"startup"}"#.to_vec(),br#"{"hook_event_name":"SessionStart","session_id":"11111111-1111-4111-8111-111111111111","source":"startup"}"#.to_vec()] {
        let mut child=command(dir.path()).spawn().unwrap();
        let _=child.stdin.take().unwrap().write_all(&input);
        let out=child.wait_with_output().unwrap();
        assert!(out.status.success());assert_eq!(out.stdout,b"{}\n");assert!(out.stderr.is_empty());
    }
}
#[test]
fn open_stdin_is_bounded_and_manual_missing_identity_fails_actionably() {
    let dir = tempfile::tempdir().unwrap();
    let start = Instant::now();
    let mut child = command(dir.path()).spawn().unwrap();
    let _keep_open = child.stdin.take().unwrap();
    let mut out = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut out)
        .unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(out, "{}\n");
    assert!(start.elapsed() < Duration::from_secs(3));
    let out = Command::new(env!("CARGO_BIN_EXE_panorama"))
        .args(["codex-bind", "11111111-1111-4111-8111-111111111111"])
        .env_clear()
        .env("HOME", dir.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--pane PANE_ID"));
    assert!(out.stdout.is_empty());
}
