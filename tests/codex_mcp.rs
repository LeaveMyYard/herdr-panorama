//! A shared MCP server must route each Codex caller independently.
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;
fn replay(requests: &[Value]) -> (Vec<Value>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_panorama"));
    cmd.env_clear()
        .arg("mcp")
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", dir.path())
        .env("PANORAMA_EVENTS_DIR", dir.path().join("events"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    cmd.env("CLAUDE_CODE_SESSION_ID", "claude-fallback")
        .env("CODEX_THREAD_ID", "33333333-3333-4333-8333-333333333333")
        .env("CODEX_SESSION_ID", "44444444-4444-4444-8444-444444444444");
    let mut child = cmd.spawn().unwrap();
    let (tx, rx) = mpsc::channel();
    let out = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            let _ = tx.send(serde_json::from_str::<Value>(&line).unwrap());
        }
    });
    let mut stdin = child.stdin.take().unwrap();
    let mut got = Vec::new();
    for r in requests {
        writeln!(stdin, "{r}").unwrap();
        stdin.flush().unwrap();
        let Some(id) = r.get("id") else { continue };
        let listen = r["method"] == "subscriptions/listen";
        loop {
            let m = rx.recv_timeout(Duration::from_secs(10)).expect("a reply");
            let done = (listen && m["method"] == "notifications/subscriptions/acknowledged")
                || m.get("id") == Some(id);
            got.push(m);
            if done {
                break;
            }
        }
    }
    drop(stdin);
    let _ = child.wait();
    (got, dir)
}

#[test]
fn alternating_root_child_calls_use_wire_metadata_without_sticky_environment() {
    let root = "11111111-1111-4111-8111-111111111111";
    let child = "22222222-2222-4222-8222-222222222222";
    let mut requests = vec![
        json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"synthetic","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    ];
    for (i, caller) in [
        json!(root),
        json!(child),
        json!(root),
        json!(null),
        json!("bad/../id"),
        json!(7),
    ]
    .into_iter()
    .enumerate()
    {
        requests.push(json!({"jsonrpc":"2.0","id":i+1,"method":"tools/call","params":{"name":"report_thread","_meta":{"threadId":caller,"sessionId":root},"arguments":{"thread_id":"same-work","title":format!("call-{i}")}}}));
    }
    requests.push(json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"report_thread","_meta":{"sessionId":root},"arguments":{"thread_id":"claude-work"}}}));
    let (got, dir) = replay(&requests);
    for id in 1..=7 {
        let msg = got.iter().find(|m| m["id"] == id).unwrap();
        if (4..=6).contains(&id) {
            assert_eq!(msg["result"]["isError"], true);
        } else {
            assert_eq!(msg["result"]["structuredContent"]["status"], "recorded");
        }
    }
    let read = |id: &str| {
        std::fs::read_to_string(dir.path().join("events").join(format!("{id}.jsonl"))).unwrap()
    };
    assert_eq!(read(root).lines().count(), 2);
    assert_eq!(read(child).lines().count(), 1);
    assert!(read(child).contains("call-1"));
    assert_eq!(read("claude-fallback").lines().count(), 1);
    assert_eq!(
        std::fs::read_dir(dir.path().join("events"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
            .count(),
        3
    );
}
