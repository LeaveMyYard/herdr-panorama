//! Explicit binding, and old per-caller reports left unread, through the changed binary, then
//! collection. Every process, socket, home, transcript and record is synthetic.

mod codex_support;

use codex_support::{CHILD, Fixture, GRANDCHILD, ROOT_A, snapshot};
use herdr_agent_kit::agent_procs::{ProcessTable, SystemProcesses};
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use serde_json::{Value, json};
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Foreground(Child);
impl Foreground {
    fn new() -> Self {
        let child = Command::new("/bin/bash")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .args(["-c", "exec -a codex sleep 30"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let owner = Self(child);
        let deadline = Instant::now() + Duration::from_secs(2);
        while SystemProcesses::new()
            .cmdline(owner.0.id())
            .first()
            .map(String::as_str)
            != Some("codex")
        {
            assert!(Instant::now() < deadline, "synthetic process did not exec");
            std::thread::sleep(Duration::from_millis(5));
        }
        owner
    }
}
impl Drop for Foreground {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// No history of the root was read: no last activity, and its one thread holds no agent.
fn unread(root: &Value) -> bool {
    root["last_activity"].is_null()
        && root["threads"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["agents"].as_array().unwrap().is_empty())
}

fn model(fixture: &Fixture, fake: &FakeHerdr) -> Value {
    let output = fixture
        .binary(fake.socket())
        .args(["snapshot", "--json"])
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
fn explicit_binding_enriches_only_current_owner_and_foreground_reuse_fails_closed() {
    let fixture = Fixture::new("complete");
    let owner = Foreground::new();
    let fake = FakeHerdr::start();
    fake.on(
        "pane.get",
        Reply::Result(json!({"pane":{"pane_id":"codex-a","agent":"codex"}})),
    );
    fake.on(
        "pane.process_info",
        Reply::Result(json!({"process_info":{
        "pane_id":"codex-a","foreground_processes":[{"pid":owner.0.id(),"name":"codex"}]}})),
    );
    let mut snapshot = snapshot();
    snapshot["agents"]
        .as_array_mut()
        .unwrap()
        .retain(|a| a["pane_id"] == "codex-a");
    snapshot["agents"][0]
        .as_object_mut()
        .unwrap()
        .remove("agent_session");
    fake.on(
        "session.snapshot",
        Reply::Result(json!({"snapshot":snapshot})),
    );
    let bound = fixture
        .binary(fake.socket())
        .args(["codex-bind", ROOT_A, "--pane", "codex-a"])
        .output()
        .unwrap();
    assert!(
        bound.status.success(),
        "{}",
        String::from_utf8_lossy(&bound.stderr)
    );
    let resolved = model(&fixture, &fake);
    assert_eq!(resolved["orchestrators"][0]["session_id"], ROOT_A);
    assert!(resolved["orchestrators"][0]["last_activity"].is_number());
    let before = fixture.calls().len();
    let record = fixture
        .root()
        .join("state/herdr-panorama/codex-bindings/636f6465782d61");
    let mut value: Value = serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
    value["conflicted"] = json!(true);
    std::fs::write(&record, serde_json::to_vec(&value).unwrap()).unwrap();
    let conflicted = model(&fixture, &fake);
    assert!(conflicted["orchestrators"][0]["session_id"].is_null());
    assert!(unread(&conflicted["orchestrators"][0]));
    assert_eq!(fixture.calls().len(), before);
    value["conflicted"] = json!(false);
    std::fs::write(&record, serde_json::to_vec(&value).unwrap()).unwrap();
    // The old process is still alive, but no longer in this pane's foreground.
    fake.on(
        "pane.process_info",
        Reply::Result(json!({"process_info":{
        "pane_id":"codex-a","foreground_processes":[]}})),
    );
    let stale = model(&fixture, &fake);
    assert!(stale["orchestrators"][0]["session_id"].is_null());
    assert!(unread(&stale["orchestrators"][0]));
    assert_eq!(
        fixture.calls().len(),
        before,
        "unproven identity never reaches the observer"
    );
    assert!(
        fake.calls().iter().all(|(method, _)| matches!(
            method.as_str(),
            "pane.get" | "pane.process_info" | "session.snapshot"
        )),
        "no lifecycle/session mutation"
    );
}

#[cfg(unix)]
#[test]
fn untrusted_binding_directory_never_authorizes_history_reads() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let fixture = Fixture::new("complete");
    let owner = Foreground::new();
    let fake = FakeHerdr::start();
    fake.on(
        "pane.get",
        Reply::Result(json!({"pane":{"pane_id":"codex-a","agent":"codex"}})),
    );
    fake.on(
        "pane.process_info",
        Reply::Result(json!({"process_info":{
        "pane_id":"codex-a","foreground_processes":[{"pid":owner.0.id(),"name":"codex"}]}})),
    );
    let mut snapshot = snapshot();
    snapshot["agents"]
        .as_array_mut()
        .unwrap()
        .retain(|a| a["pane_id"] == "codex-a");
    snapshot["agents"][0]
        .as_object_mut()
        .unwrap()
        .remove("agent_session");
    fake.on(
        "session.snapshot",
        Reply::Result(json!({"snapshot":snapshot})),
    );
    let bound = fixture
        .binary(fake.socket())
        .args(["codex-bind", ROOT_A, "--pane", "codex-a"])
        .output()
        .unwrap();
    assert!(
        bound.status.success(),
        "{}",
        String::from_utf8_lossy(&bound.stderr)
    );
    let before = fixture.calls().len();
    let directory = fixture.root().join("state/herdr-panorama/codex-bindings");
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o777)).unwrap();
    let untrusted = model(&fixture, &fake);
    assert!(untrusted["orchestrators"][0]["session_id"].is_null());
    assert_eq!(fixture.calls().len(), before);
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    let displaced = directory.with_file_name("displaced-bindings");
    std::fs::rename(&directory, &displaced).unwrap();
    symlink(&displaced, &directory).unwrap();
    let untrusted = model(&fixture, &fake);
    assert!(untrusted["orchestrators"][0]["session_id"].is_null());
    assert_eq!(fixture.calls().len(), before);
}

#[test]
fn old_per_caller_reports_are_not_read_and_mcp_writes_none() {
    let fixture = Fixture::new("complete");
    let fake = FakeHerdr::start();
    fake.on(
        "session.snapshot",
        Reply::Result(json!({"snapshot":snapshot()})),
    );
    let mut child = fixture
        .binary(fake.socket())
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for request in [
        json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"synthetic","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"report_thread","_meta":{"threadId":CHILD,"sessionId":ROOT_A},"arguments":{"thread_id":"child-plan","title":"Synthetic child plan"}}}),
    ] {
        writeln!(input, "{request}").unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let replies: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    // threads were removed: the call is ignored and nothing is written
    assert!(replies.iter().any(
        |reply| reply["id"] == 1 && reply["result"]["structuredContent"]["status"] == "ignored"
    ));
    let events = fixture.root().join("events");
    assert!(!events.join(format!("{CHILD}.jsonl")).exists());
    // what an earlier version wrote for that call is no longer read (plan
    // `uniform-agent-tree.md` §6): the child's subtree is the root's tree as Codex proves it
    std::fs::create_dir_all(&events).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let written = json!({"v": 1, "ts": now, "thread": "child-plan", "name": "Synthetic child plan", "status": "active"});
    std::fs::write(
        events.join(format!("{CHILD}.jsonl")),
        format!("{written}\n"),
    )
    .unwrap();
    let mut collector = fixture.collector(true);
    let resolved = serde_json::to_value(codex_support::ready(&mut collector, &snapshot())).unwrap();
    let root = &resolved["orchestrators"][1];
    assert_eq!(root["session_id"], ROOT_A);
    let [thread] = root["threads"].as_array().unwrap().as_slice() else {
        panic!("one thread: {root}")
    };
    assert_eq!(thread["id"], panorama_core::model::TREE_THREAD);
    let agents: Vec<(&str, &Value)> = thread["agents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| (a["id"].as_str().unwrap(), &a["parent"]))
        .collect();
    assert_eq!(agents, [(CHILD, &Value::Null), (GRANDCHILD, &json!(CHILD))]);
    assert!(!resolved.to_string().contains("child-plan"));
}
