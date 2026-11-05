use super::*;
use crate::host::link::{Link, combine};
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
        ts: 1.0,
    }
}

fn hooks(records: &[Record]) -> Hooks {
    Hooks {
        records: records.to_vec(),
        complete: true,
    }
}

#[test]
fn recorded_terminal_and_current_owned_process_select_one_exact_session() {
    let info = json!({"process_info":{"pane_id":"core:t2","foreground_processes":[{"pid":20}]}});
    let mut table = FakeProcesses::default();
    table.insert(20, 1, "claude", "boot-1", &["claude"]);
    assert_eq!(
        claude_session(&hooks(&[record("session-a")]), "t2", &info, &table),
        Signal::Names("session-a".into())
    );
    assert!(claude_session(&hooks(&[record("session-a")]), "t3", &info, &table) == Signal::Absent);
    // Two sessions with one clock reading cannot be ordered: ambiguous, never a guess.
    assert_eq!(
        claude_session(
            &hooks(&[record("session-a"), record("session-b")]),
            "t2",
            &info,
            &table
        ),
        Signal::Ambiguous
    );
    table.insert(20, 1, "claude", "reused-pid", &["claude"]);
    assert!(claude_session(&hooks(&[record("session-a")]), "t2", &info, &table) == Signal::Absent);
    // The same PID and start token after an exec away from Claude.
    table.insert(20, 1, "bash", "boot-1", &["bash"]);
    assert!(claude_session(&hooks(&[record("session-a")]), "t2", &info, &table) == Signal::Absent);
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
    let found = load(dir.path()).records;
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
    assert!(claude_session(&hooks(&[rec]), "t2", &info, &table) == Signal::Absent);
    assert!(namespace_matches(None));
    if let Some(mine) = herdr_agent_kit::agent_procs::own_pidns() {
        assert!(namespace_matches(Some(&mine)));
    }
}

#[test]
fn several_sessions_of_one_process_keep_only_the_newest_for_corroboration() {
    let info = json!({"process_info":{"pane_id":"core:t2","foreground_processes":[{"pid":20}]}});
    let mut table = FakeProcesses::default();
    table.insert(20, 1, "claude", "boot-1", &["claude"]);
    let mut newer = record("session-b");
    newer.ts = 2.0;
    let hook = claude_session(
        &hooks(&[record("session-a"), newer.clone()]),
        "t2",
        &info,
        &table,
    );
    assert_eq!(hook, Signal::Newest("session-b".into()));
    // Alone it never links: the hook may have dropped a later record.
    assert_eq!(combine(&[hook.clone(), Signal::Absent]), Link::Unavailable);
    assert_eq!(
        combine(&[hook.clone(), Signal::Names("session-b".into())]),
        Link::Linked("session-b".into())
    );
    assert_eq!(
        combine(&[hook, Signal::Names("session-a".into())]),
        Link::Conflict
    );
    let mut unordered = record("session-c");
    unordered.ts = f64::NAN;
    assert_eq!(
        claude_session(
            &hooks(&[record("session-a"), newer, unordered]),
            "t2",
            &info,
            &table
        ),
        Signal::Ambiguous
    );
}

#[test]
fn a_hook_file_left_unread_keeps_one_session_from_naming_itself() {
    let info = json!({"process_info":{"pane_id":"core:t2","foreground_processes":[{"pid":20}]}});
    let mut table = FakeProcesses::default();
    table.insert(20, 1, "claude", "boot-1", &["claude"]);
    let partial = |records: &[Record]| Hooks {
        records: records.to_vec(),
        complete: false,
    };
    // Kept for corroboration, as with several sessions.
    assert_eq!(
        claude_session(&partial(&[record("session-a")]), "t2", &info, &table),
        Signal::Newest("session-a".into())
    );
    // Nothing of this process was read: nothing is claimed about it.
    assert_eq!(
        claude_session(&partial(&[]), "t2", &info, &table),
        Signal::Absent
    );
    let mut unordered = record("session-a");
    unordered.ts = f64::NAN;
    assert_eq!(
        claude_session(&partial(&[unordered]), "t2", &info, &table),
        Signal::Ambiguous
    );
}
