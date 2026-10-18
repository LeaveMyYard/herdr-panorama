use super::*;
use std::path::Path;

const FIX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/claude-hooks");

/// A captured payload (anonymised), as Claude Code 2.1.302 sent it.
fn captured(name: &str) -> String {
    let doc: Value = serde_json::from_str(
        &std::fs::read_to_string(Path::new(FIX).join(format!("{name}.json"))).unwrap(),
    )
    .unwrap();
    doc["stdin"].to_string()
}

fn cx(dir: &Path, in_herdr: bool) -> Ctx {
    Ctx {
        hooks_dir: dir.join("hooks"),
        in_herdr,
        now: 1_791_960_000.5,
        identity: Box::new(|| None),
    }
}

fn hook(kind: &str, payload: &str, cx: &Ctx) -> (i32, String) {
    let mut out = Vec::new();
    let rc = run(kind, &mut payload.as_bytes(), &mut out, cx);
    (rc, String::from_utf8(out).unwrap())
}

fn records(cx: &Ctx, session: &str) -> Vec<Value> {
    let path = cx.hooks_dir.join(format!("{session}.jsonl"));
    std::fs::read_to_string(path).map_or_else(
        |_| vec![],
        |t| {
            t.lines()
                .map(|l| serde_json::from_str(l).unwrap())
                .collect()
        },
    )
}

#[test]
fn start_and_stop_are_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let c = cx(dir.path(), false);
    let (rc, out) = hook(
        "subagent-start",
        r#"{"session_id": "s1", "agent_id": "agent-abc", "agent_type": "Explore"}"#,
        &c,
    );
    assert_eq!((rc, out.as_str()), (0, ""));
    let stop = r#"{"session_id": "s1", "agent_id": "abc", "agent_type": "Explore", "agent_transcript_path": "/x", "last_assistant_message": "done", "stop_hook_active": false}"#;
    hook("subagent-stop", stop, &c);
    let recs = records(&c, "s1");
    let got: Vec<_> = recs
        .iter()
        .map(|r| {
            (
                r["event"].as_str().unwrap(),
                r["agent"].as_str().unwrap(),
                r["agent_type"].as_str().unwrap(),
                r["v"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            ("start", "abc", "Explore", 1),
            ("stop", "abc", "Explore", 1)
        ]
    );
    assert!(recs.iter().all(|r| r["ts"].is_f64()));
    let text = std::fs::read_to_string(c.hooks_dir.join("s1.jsonl")).unwrap();
    assert!(text.starts_with("{\"v\": 1, \"event\": \"start\", \"agent\": \"abc\", \"agent_type\": \"Explore\", \"ts\": 1791960000.5}\n"), "{text}");
}

#[test]
fn captured_payloads_are_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = cx(dir.path(), true);
    c.identity = Box::new(|| Some((4242, "123456".into(), Some("pid:[4026531836]".into()))));
    let session = "00000001-0000-4000-8000-000000000001";
    assert_eq!(
        hook(
            "subagent-start",
            &captured("SubagentStart.panorama-matcher-group"),
            &c
        ),
        (0, String::new())
    );
    assert_eq!(
        hook("subagent-stop", &captured("SubagentStop"), &c),
        (0, String::new())
    );
    let recs = records(&c, session);
    assert_eq!(recs.len(), 2);
    for r in &recs {
        assert_eq!(r["agent"], "abbbbbbbbbbbbbbb3"); // `agent_id` as sent: no "agent-" prefix
        assert_eq!(r["agent_type"], "general-purpose");
        assert_eq!(
            (r["pid"].as_i64(), r["pstart"].as_str(), r["pidns"].as_str()),
            (Some(4242), Some("123456"), Some("pid:[4026531836]"))
        );
    }
    let (rc, out) = hook("session-start", &captured("SessionStart.startup"), &c);
    assert_eq!(rc, 0);
    let out: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(out["hookSpecificOutput"]["hookEventName"], "SessionStart");
    let session_rec = &records(&c, session)[2];
    assert_eq!(
        (
            session_rec["event"].as_str(),
            session_rec["source"].as_str(),
            session_rec["told"].as_bool()
        ),
        (Some("session"), Some("startup"), Some(true))
    );
    assert_eq!(
        hook("session-start", &captured("SessionStart.compact"), &c).1,
        ""
    ); // told once
}

#[test]
fn bad_input_never_fails_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let c = cx(dir.path(), true);
    for payload in [
        "{bad",
        "[1]",
        "",
        r#"{"session_id": "s1"}"#,
        r#"{"agent_id": "a"}"#,
        r#"{"session_id": "", "agent_id": "a"}"#,
    ] {
        assert_eq!(
            hook("subagent-stop", payload, &c),
            (0, String::new()),
            "{payload}"
        );
    }
    assert!(!c.hooks_dir.join("s1.jsonl").exists());
    let blocker = dir.path().join("file");
    std::fs::write(&blocker, "x").unwrap();
    let blocked = Ctx {
        hooks_dir: blocker.join("sub"),
        ..cx(dir.path(), true)
    }; // cannot be created
    assert_eq!(
        hook(
            "subagent-start",
            r#"{"session_id": "s1", "agent_id": "a"}"#,
            &blocked
        ),
        (0, String::new())
    );
    assert_eq!(
        hook("session-start", r#"{"session_id": "s1"}"#, &blocked),
        (0, String::new())
    );
}

#[test]
fn session_start_text_only_inside_herdr() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        hook(
            "session-start",
            r#"{"session_id": "s0", "source": "startup"}"#,
            &cx(dir.path(), false)
        )
        .1,
        ""
    );
    let out = hook(
        "session-start",
        r#"{"session_id": "s7", "source": "startup"}"#,
        &cx(dir.path(), true),
    )
    .1;
    let out: Value = serde_json::from_str(&out).unwrap();
    let ctx = out["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert_eq!(out["hookSpecificOutput"]["hookEventName"], "SessionStart");
    assert!(ctx.contains("report_thread") && ctx.split_whitespace().count() < 70); // short
}

#[test]
fn l4_session_text_once_per_session() {
    let dir = tempfile::tempdir().unwrap();
    let c = cx(dir.path(), true);
    let first = hook(
        "session-start",
        r#"{"session_id": "s1", "source": "startup"}"#,
        &c,
    )
    .1;
    let again = hook(
        "session-start",
        r#"{"session_id": "s1", "source": "compact"}"#,
        &c,
    )
    .1;
    let other = hook(
        "session-start",
        r#"{"session_id": "s2", "source": "startup"}"#,
        &c,
    )
    .1;
    assert!(first.contains("report_thread") && again.is_empty() && other.contains("report_thread"));
}

#[test]
fn stop_hook_active_is_respected_and_unknown_kinds_do_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let c = cx(dir.path(), true);
    hook(
        "subagent-stop",
        r#"{"session_id": "s1", "agent_id": "a", "stop_hook_active": true}"#,
        &c,
    );
    assert!(!c.hooks_dir.join("s1.jsonl").exists());
    assert_eq!(
        hook("bogus", r#"{"session_id": "s1", "agent_id": "a"}"#, &c),
        (0, String::new())
    );
    assert!(!c.hooks_dir.exists());
}

#[test]
fn old_hook_files_are_pruned() {
    let dir = tempfile::tempdir().unwrap();
    let c = cx(dir.path(), false);
    std::fs::create_dir_all(&c.hooks_dir).unwrap();
    let (old, new, other) = (
        c.hooks_dir.join("old.jsonl"),
        c.hooks_dir.join("new.jsonl"),
        c.hooks_dir.join("old.txt"),
    );
    for p in [&old, &new, &other] {
        std::fs::write(p, "{}\n").unwrap();
    }
    let then = std::time::UNIX_EPOCH + std::time::Duration::from_secs_f64(c.now - 30.0 * 86_400.0);
    for p in [&old, &other] {
        std::fs::File::options()
            .write(true)
            .open(p)
            .unwrap()
            .set_modified(then)
            .unwrap();
    }
    hook(
        "session-start",
        r#"{"session_id": "s1", "source": "startup"}"#,
        &c,
    );
    assert!(!old.exists() && new.exists() && other.exists());
}

#[test]
fn concurrent_hooks_keep_every_line_whole() {
    let dir = tempfile::tempdir().unwrap();
    let hooks_dir = dir.path().join("hooks");
    let threads: Vec<_> = (0..16)
        .map(|i| {
            let hooks_dir = hooks_dir.clone();
            std::thread::spawn(move || {
                let c = Ctx {
                    hooks_dir,
                    in_herdr: false,
                    now: 1.0,
                    identity: Box::new(|| None),
                };
                let payload =
                    format!(r#"{{"session_id": "s", "agent_id": "a{i}", "agent_type": "x"}}"#);
                hook("subagent-stop", &payload, &c)
            })
        })
        .collect();
    for t in threads {
        assert_eq!(t.join().unwrap(), (0, String::new()));
    }
    let text = std::fs::read_to_string(hooks_dir.join("s.jsonl")).unwrap();
    let agents: std::collections::BTreeSet<String> = text
        .lines()
        .map(|l| {
            serde_json::from_str::<Value>(l).unwrap()["agent"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(agents.len(), 16);
}
