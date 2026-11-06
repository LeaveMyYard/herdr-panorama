//! Agent resume through the real writer, over a recorded `hc` world and a fake app.
mod app_feed_support;
use app_feed_support::{ID, Installed};
use serde_json::{Value, json};
use std::{os::unix::fs::PermissionsExt, path::Path, time::Duration};

const SESSION: &str = "11111111-2222-4333-8444-555555555555";
const LOST_HOLDER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CORE: &str = "0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f";

fn write(path: &Path, bytes: &[u8], mode: u32) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

fn world() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/hc-resume-recovered.json"
    );
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// A fake `hc` that answers from `world.json` and logs each call with its caller variable.
fn serve(install: &Installed, world: &Value) {
    write(
        &install.data.join("world.json"),
        &serde_json::to_vec(world).unwrap(),
        0o600,
    );
    let root = serde_json::to_string(&install.data.to_string_lossy()).unwrap();
    let hc = format!(
        r#"#!/usr/bin/env python3
import json,os,sys,pathlib,time
root=pathlib.Path({root})
args=sys.argv[1:]
with (root/'calls').open('a') as out: out.write(json.dumps({{'args':args,'caller':os.environ.get('HC_TERMINAL_ID')}})+'\n')
world=json.loads((root/'world.json').read_text())
if args==['ls','--json']: print(json.dumps({{'hidden_finished':0,'terminals':[]}}))
elif args==['events','--json']: time.sleep(600)
elif args==['server','status','--json']: print(json.dumps(world['status']))
elif args[:2]==['get','--json']: print(json.dumps(world['get']))
elif args[:2]==['read','--json']: print(json.dumps(world['read']))
elif args[:2]==['send','--json']: print(json.dumps({{'started':None,'status':None,'turn':0}}))
else: raise SystemExit(64)
"#
    );
    write(&install.bin.join("hc"), hc.as_bytes(), 0o700);
}

fn toggle_on(install: &Installed) {
    let store = json!({"agents.resume": "\"on\""}).to_string();
    write(
        &install.app.join("data/panorama-spaces/store.json"),
        store.as_bytes(),
        0o600,
    );
}

/// What a previous writer left: a proved session in t5, under a holder that is `holder`.
fn seed(install: &Installed, holder: &str, closed: bool) {
    let record = json!({
        "address":"t5","harness":"claude","session":SESSION,"cwd":"/fixture/project-alpha",
        "root_pid":20,"agent_pid":30,"created_ms":world()["get"]["created_ms"],
        "launch":"plain","proved":true,"ending_ms":null});
    let saved = json!({"v":1,"install_id":ID,"core":CORE,"holders":[holder],"closed":closed,
        "records":{"t5":record},"loss":null});
    write(
        &resume_path(install),
        &serde_json::to_vec(&saved).unwrap(),
        0o600,
    );
}

fn resume_path(install: &Installed) -> std::path::PathBuf {
    install
        .app
        .join(format!("data/panorama/runtime/{ID}/resume.json"))
}

fn calls(install: &Installed) -> Vec<Value> {
    std::fs::read_to_string(install.data.join("calls"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn sends(install: &Installed) -> Vec<Value> {
    calls(install)
        .into_iter()
        .filter(|c| c["args"][0] == "send")
        .collect()
}

fn state(feed: &Value) -> String {
    feed["resume"]["loss"]["entries"][0]["state"]
        .as_str()
        .unwrap_or("")
        .to_owned()
}

#[test]
fn off_by_default_the_writer_makes_no_new_call_and_reports_nothing() {
    let install = Installed::new();
    let mut lost = world();
    lost["get"]["address"] = json!("t5");
    serve(&install, &lost);
    // A record left from before it was turned off must not act, and must go.
    seed(&install, LOST_HOLDER, false);
    assert!(install.ensure().status.success());
    install.wait_feed(|v| v["v"] == 2 && !resume_path(&install).exists());
    std::thread::sleep(Duration::from_millis(600));
    let seen = calls(&install);
    assert!(!seen.is_empty());
    for call in &seen {
        let verb = call["args"][0].as_str().unwrap();
        assert!(matches!(verb, "ls" | "events"), "{call}");
    }
    assert_eq!(install.feed().get("resume"), None);
    let _ = install.quiesce().unwrap();
}

#[test]
fn a_recorded_loss_is_typed_once_through_the_real_writer() {
    let install = Installed::new();
    serve(&install, &world());
    toggle_on(&install);
    seed(&install, LOST_HOLDER, false);
    let start = install
        .command()
        .env("HC_TERMINAL_ID", "t9")
        .args(["app-feed", "--ensure", "--app-home"])
        .arg(&install.app)
        .output()
        .unwrap();
    assert!(start.status.success());
    install.wait_feed(|v| state(v) == "sent");
    let sent = sends(&install);
    assert_eq!(sent.len(), 1);
    let typed = format!("claude --resume {SESSION}");
    assert_eq!(sent[0]["args"], json!(["send", "--json", "t5", typed]));
    assert_eq!(
        sent[0]["caller"],
        Value::Null,
        "no caller, so hc adds no tag"
    );
    let entry = install.feed()["resume"]["loss"]["entries"][0].clone();
    assert_eq!(entry["terminal"], "t5");
    assert_eq!(entry["command"], typed);
    // Staying at a shell afterwards is no reason to type again.
    std::thread::sleep(Duration::from_millis(2500));
    assert_eq!(sends(&install).len(), 1);
    let (_home, _r, _lock) = install.quiesce().unwrap();
}

#[test]
fn the_same_holders_type_nothing_through_the_real_writer() {
    let install = Installed::new();
    let world = world();
    serve(&install, &world);
    toggle_on(&install);
    seed(
        &install,
        world["status"]["holders"][0]["instance"].as_str().unwrap(),
        false,
    );
    assert!(install.ensure().status.success());
    install.wait_feed(|v| v["resume"]["tracked"] == 1);
    std::thread::sleep(Duration::from_millis(2500));
    assert!(sends(&install).is_empty());
    assert_eq!(install.feed()["resume"]["loss"], Value::Null);
    let _ = install.quiesce().unwrap();
}

#[test]
fn a_loss_after_a_clean_close_is_shown_with_its_command_and_not_typed() {
    let install = Installed::new();
    serve(&install, &world());
    toggle_on(&install);
    seed(&install, LOST_HOLDER, true);
    assert!(install.ensure().status.success());
    install.wait_feed(|v| state(v) == "skipped");
    std::thread::sleep(Duration::from_millis(2500));
    assert!(sends(&install).is_empty());
    let entry = install.feed()["resume"]["loss"]["entries"][0].clone();
    assert_eq!(entry["command"], format!("claude --resume {SESSION}"));
    assert!(entry["reason"].as_str().unwrap().contains("closed"));
    let _ = install.quiesce().unwrap();
}

#[test]
fn a_writer_that_stops_on_purpose_marks_the_record_closed() {
    let install = Installed::new();
    let world = world();
    serve(&install, &world);
    toggle_on(&install);
    seed(
        &install,
        world["status"]["holders"][0]["instance"].as_str().unwrap(),
        false,
    );
    assert!(install.ensure().status.success());
    install.wait_feed(|v| v["resume"]["tracked"] == 1);
    let (_home, _r, _lock) = install.quiesce().unwrap();
    let saved: Value =
        serde_json::from_slice(&std::fs::read(resume_path(&install)).unwrap()).unwrap();
    assert_eq!(saved["closed"], true);
}
