use super::*;
use herdr_agent_kit::agent_procs::fake::FakeProcesses;
use serde_json::json;

fn record(session: &str) -> Record {
    Record {
        session: session.into(),
        terminal: "t2".into(),
        harness: "claude".into(),
        pid: 20,
        start: "boot-1".into(),
        pidns: None,
    }
}

#[test]
fn recorded_terminal_and_current_owned_process_select_one_exact_session() {
    let info = json!({"process_info":{"pane_id":"core:t2","foreground_processes":[{"pid":20}]}});
    let mut table = FakeProcesses::default();
    table.insert(20, 1, "claude", "boot-1", &["claude"]);
    assert_eq!(
        claude_session(&[record("session-a")], "t2", &info, &table).as_deref(),
        Some("session-a")
    );
    assert!(claude_session(&[record("session-a")], "t3", &info, &table).is_none());
    assert!(
        claude_session(
            &[record("session-a"), record("session-b")],
            "t2",
            &info,
            &table
        )
        .is_none()
    );
    table.insert(20, 1, "claude", "reused-pid", &["claude"]);
    assert!(claude_session(&[record("session-a")], "t2", &info, &table).is_none());
}

#[test]
fn old_records_and_subagent_or_remote_records_cannot_acquire_a_core_address() {
    let dir = tempfile::tempdir().unwrap();
    let base = json!({"v":1,"event":"session","host":"herdr-core","terminal":"t2","harness":"claude","pid":20,"pstart":"boot-1"});
    let mut lines = vec![base.clone()];
    for (key, value) in [
        ("host", json!(null)),
        ("event", json!("start")),
        ("terminal", json!("vps/t2")),
        ("pid", json!(0)),
    ] {
        let mut line = base.clone();
        line[key] = value;
        lines.push(line);
    }
    std::fs::write(
        dir.path()
            .join("00000000-0000-4000-8000-000000000000.jsonl"),
        lines
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    let found = load(dir.path());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].terminal, "t2");
}

#[test]
fn foreign_namespace_cannot_match_a_local_pid_and_start_token() {
    let info = json!({"process_info":{"pane_id":"core:t2","foreground_processes":[{"pid":20}]}});
    let mut table = FakeProcesses::default();
    table.insert(20, 1, "claude", "boot-1", &["claude"]);
    let mut rec = record("session-a");
    rec.pidns = Some("pid:[foreign-namespace]".into());
    assert!(claude_session(&[rec], "t2", &info, &table).is_none());
    assert!(namespace_matches(None));
    if let Some(mine) = herdr_agent_kit::agent_procs::own_pidns() {
        assert!(namespace_matches(Some(&mine)));
    }
}
