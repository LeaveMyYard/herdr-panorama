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

fn cx(dir: &Path) -> Ctx {
    Ctx {
        host: crate::host::Context {
            host: crate::host::Host::Unknown,
            terminal: None,
            pane_id: None,
        },
        harness: "claude",
        hooks_dir: dir.join("hooks"),
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
    let c = cx(dir.path());
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
    let mut c = cx(dir.path());
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
    // the hook payload golden: SessionStart says nothing to Claude Code (no additionalContext)
    assert_eq!(
        hook("session-start", &captured("SessionStart.startup"), &c),
        (0, String::new())
    );
    let session_rec = &records(&c, session)[2];
    assert_eq!(
        (
            session_rec["event"].as_str(),
            session_rec["source"].as_str(),
            session_rec["told"].as_bool()
        ),
        (Some("session"), Some("startup"), Some(false))
    );
    assert_eq!(
        hook("session-start", &captured("SessionStart.compact"), &c),
        (0, String::new())
    );
    assert_eq!(records(&c, session)[3]["source"], "compact");
}

#[test]
fn bad_input_never_fails_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let c = cx(dir.path());
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
        host: crate::host::Context {
            host: crate::host::Host::Unknown,
            terminal: None,
            pane_id: None,
        },
        harness: "claude",
        hooks_dir: blocker.join("sub"),
        ..cx(dir.path())
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
fn session_start_says_nothing_and_still_records_the_session() {
    // inside herdr or not (the hook no longer asks): nothing on stdout, the record written
    let dir = tempfile::tempdir().unwrap();
    let c = cx(dir.path());
    let payload = r#"{"session_id": "s7", "source": "startup"}"#;
    assert_eq!(hook("session-start", payload, &c), (0, String::new()));
    let recs = records(&c, "s7");
    assert_eq!(recs.len(), 1);
    assert_eq!(
        (&recs[0]["event"], &recs[0]["told"]),
        (&Value::from("session"), &Value::from(false))
    );
}

#[test]
fn every_session_start_is_recorded_and_none_is_told() {
    let dir = tempfile::tempdir().unwrap();
    let c = cx(dir.path());
    for (sid, source) in [("s1", "startup"), ("s1", "compact"), ("s2", "startup")] {
        let payload = format!(r#"{{"session_id": "{sid}", "source": "{source}"}}"#);
        assert_eq!(hook("session-start", &payload, &c), (0, String::new()));
    }
    let told = |sid| -> Vec<bool> {
        records(&c, sid)
            .iter()
            .map(|r| r["told"].as_bool().unwrap())
            .collect()
    };
    assert_eq!((told("s1"), told("s2")), (vec![false, false], vec![false]));
}

#[test]
fn stop_hook_active_is_respected_and_unknown_kinds_do_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let c = cx(dir.path());
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
    let c = cx(dir.path());
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
                    host: crate::host::Context {
                        host: crate::host::Host::Unknown,
                        terminal: None,
                        pane_id: None,
                    },
                    harness: "claude",
                    hooks_dir,
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

#[test]
fn core_records_keep_terminal_and_pane_beside_existing_process_identity() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = cx(dir.path());
    c.host = crate::host::Context {
        host: crate::host::Host::Core,
        terminal: Some("t2".into()),
        pane_id: Some("w1:p1".into()),
    };
    c.identity = Box::new(|| Some((4242, "start".into(), Some("pidns-fixture".into()))));
    hook("session-start", &captured("SessionStart.startup"), &c);
    let recs = records(&c, "00000001-0000-4000-8000-000000000001");
    assert_eq!(recs.len(), 1);
    let r = &recs[0];
    assert_eq!(r["host"], "herdr-core");
    assert_eq!(r["terminal"], "t2");
    assert_eq!(r["pane_id"], "w1:p1");
    assert_eq!(r["harness"], "claude");
    assert_eq!(r["pid"], 4242);
    assert_eq!(r["pstart"], "start");
    assert_eq!(r["pidns"], "pidns-fixture");
    for marker in ["agent_id", "parent_session_id"] {
        let mut payload: Value = serde_json::from_str(&captured("SessionStart.startup")).unwrap();
        payload[marker] = serde_json::json!("child");
        assert_eq!(
            hook("session-start", &payload.to_string(), &c),
            (0, String::new())
        );
    }
    assert_eq!(records(&c, "00000001-0000-4000-8000-000000000001").len(), 1);
}

#[test]
fn a_lone_surrogate_escape_never_loses_a_record() {
    // Node cuts a string inside an emoji: Claude Code sends `\ud83d` with no partner
    let dir = tempfile::tempdir().unwrap();
    let c = cx(dir.path());
    let stop = r#"{"session_id": "s1", "agent_id": "a1", "agent_type": "Explore", "last_assistant_message": "done \ud83d"}"#;
    let start = r#"{"session_id": "s1", "agent_id": "a1", "agent_type": "x\udc00"}"#;
    assert_eq!(hook("subagent-start", start, &c), (0, String::new()));
    assert_eq!(hook("subagent-stop", stop, &c), (0, String::new()));
    let recs = records(&c, "s1");
    let got: Vec<_> = recs
        .iter()
        .map(|r| {
            (
                r["event"].as_str().unwrap(),
                r["agent_type"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(got, [("start", "x\u{fffd}"), ("stop", "Explore")]);
    let said = hook(
        "session-start",
        r#"{"session_id": "s1", "source": "startup \ud83d"}"#,
        &c,
    )
    .1;
    assert!(said.is_empty());
    assert_eq!(records(&c, "s1")[2]["source"], "startup \u{fffd}");
}

#[test]
fn a_bom_is_refused_as_python_refuses_it() {
    let dir = tempfile::tempdir().unwrap();
    let c = cx(dir.path());
    assert_eq!(
        hook(
            "subagent-stop",
            "\u{feff}{\"session_id\": \"s1\", \"agent_id\": \"a\"}",
            &c
        ),
        (0, String::new())
    );
    assert!(records(&c, "s1").is_empty());
    assert!(payload("  {\"a\": 1}\r\n").contains_key("a"));
    assert!(payload("[1]").is_empty());
}

#[test]
fn setup_prints_the_three_hooks_with_the_binary_quoted_for_sh() {
    let value = super::setup::snippet("/opt/it's here/panorama");
    let hooks = value["hooks"].as_object().unwrap();
    assert_eq!(hooks.len(), 3);
    let start = &hooks["SessionStart"][0];
    assert_eq!(start["matcher"], "startup|resume|clear|compact|fork");
    assert_eq!(
        start["hooks"][0]["command"],
        r"'/opt/it'\''s here/panorama' hook session-start"
    );
    assert_eq!(start["hooks"][0]["type"], "command");
    assert_eq!(
        hooks["SubagentStop"][0]["hooks"][0]["command"],
        r"'/opt/it'\''s here/panorama' hook subagent-stop"
    );
    // The same matcher the plugin's own hooks file uses.
    let shipped: serde_json::Value =
        serde_json::from_str(include_str!("../../hooks/hooks.json")).unwrap();
    assert_eq!(
        shipped["hooks"]["SessionStart"][0]["matcher"],
        start["matcher"]
    );
}
