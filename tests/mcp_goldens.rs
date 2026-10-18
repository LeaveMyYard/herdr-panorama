//! `panorama mcp` against the Python server's recordings (`tests/fixtures/mcp/*.jsonl`,
//! `tools/capture_mcp.py`): each session's client lines are sent to the binary (environment
//! cleared, events in a temp dir) and its replies compared with the Python server's.
//!
//! The tool's list entry and every `report_thread` result and events record are the Python
//! server's exactly. Where rmcp's envelope differs (stated in `src/mcp/mod.rs`), the parts a
//! client reads are compared: an unknown method's code, a refused call's `isError`, and the
//! advertised capabilities, instructions and server info.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const SID: &str = "0e4a6b1c-9d2f-4a3b-8c7d-1e2f3a4b5c6d";

fn golden(name: &str) -> Vec<Value> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mcp")
        .join(format!("{name}.jsonl"));
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn of(recs: &[Value], dir: &str) -> Vec<Value> {
    recs.iter()
        .filter(|r| r["dir"] == dir)
        .map(|r| r["msg"].clone())
        .collect()
}

/// The binary's replies to `requests`, and its events dir.
fn replay(requests: &[Value], session: bool) -> (Vec<Value>, tempfile::TempDir) {
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
    if session {
        cmd.env("CLAUDE_CODE_SESSION_ID", SID);
    }
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

fn by_id<'a>(msgs: &'a [Value], id: &Value) -> &'a Value {
    msgs.iter()
        .find(|m| m.get("id") == Some(id) && m.get("method").is_none())
        .unwrap_or_else(|| panic!("no reply {id}"))
}

/// The capabilities, instructions and server name an `initialize` or discovery reply gives.
fn advertised(m: &Value) -> Value {
    let r = &m["result"];
    let info = r
        .get("serverInfo")
        .cloned()
        .unwrap_or_else(|| r["_meta"]["io.modelcontextprotocol/serverInfo"].clone());
    json!([
        r["capabilities"],
        r["instructions"],
        info,
        r.get("protocolVersion"),
        r.get("supportedVersions")
    ])
}

#[test]
fn the_legacy_handshake_lists_the_same_tool_and_refuses_the_same_way() {
    let recs = golden("legacy");
    let (got, _dir) = replay(&of(&recs, "client->server"), true);
    let want = of(&recs, "server->client");
    assert_eq!(
        advertised(by_id(&got, &json!(0))),
        advertised(by_id(&want, &json!(0)))
    );
    assert_eq!(by_id(&got, &json!(1)), by_id(&want, &json!(1))); // tools/list, word for word
    assert_eq!(
        by_id(&got, &json!(2))["error"]["code"],
        by_id(&want, &json!(2))["error"]["code"]
    );
    assert_eq!(by_id(&got, &json!(3)), by_id(&want, &json!(3))); // an unknown tool
    assert_eq!(by_id(&got, &json!(4)), by_id(&want, &json!(4))); // ping
}

#[test]
fn claude_code_s_2026_requests_get_the_python_server_s_answers() {
    let recs = golden("claude-code");
    let (got, _dir) = replay(&of(&recs, "client->server"), true);
    let want = of(&recs, "server->client");
    for id in [json!("server-discover-probe-1"), json!(0)] {
        assert_eq!(
            advertised(by_id(&got, &id)),
            advertised(by_id(&want, &id)),
            "{id}"
        );
    }
    let ack = |msgs: &[Value]| {
        msgs.iter()
            .find(|m| m["method"] == "notifications/subscriptions/acknowledged")
            .cloned()
    };
    assert_eq!(ack(&got), ack(&want));
    assert_eq!(by_id(&got, &json!(1))["result"]["prompts"], json!([]));
    assert_eq!(by_id(&got, &json!(2))["result"]["resources"], json!([]));
    assert_eq!(
        by_id(&got, &json!(3))["result"]["tools"],
        by_id(&want, &json!(3))["result"]["tools"]
    );
}

/// An events record with its times replaced, as the capture writes them.
fn normalised(mut rec: Value) -> Value {
    rec["ts"] = json!("<ts>");
    for ph in rec["phases"].as_array_mut().into_iter().flatten() {
        for k in ["start", "end"] {
            if ph.get(k).is_some() {
                ph[k] = json!("<ts>");
            }
        }
    }
    rec
}

#[test]
fn report_thread_answers_and_records_as_the_python_server_does() {
    let recs = golden("report-thread");
    let (got, dir) = replay(&of(&recs, "client->server"), true);
    let want = of(&recs, "server->client");
    assert_eq!(
        advertised(by_id(&got, &json!(0))),
        advertised(by_id(&want, &json!(0)))
    );
    for id in (1..=9).filter(|i| *i != 7) {
        assert_eq!(
            by_id(&got, &json!(id)),
            by_id(&want, &json!(id)),
            "call {id}"
        );
    }
    // no thread_id: refused (in rmcp's words, not pydantic's)
    assert_eq!(by_id(&got, &json!(7))["result"]["isError"], json!(true));
    let text =
        std::fs::read_to_string(dir.path().join("events").join(format!("{SID}.jsonl"))).unwrap();
    let events: Vec<Value> = text
        .lines()
        .map(|l| normalised(serde_json::from_str(l).unwrap()))
        .collect();
    assert_eq!(events, of(&recs, "events"));
}

#[test]
fn without_a_session_nothing_is_recorded() {
    let recs = golden("no-session");
    let (got, dir) = replay(&of(&recs, "client->server"), false);
    let want = of(&recs, "server->client");
    assert_eq!(by_id(&got, &json!(1)), by_id(&want, &json!(1)));
    assert!(!dir.path().join("events").exists());
}
