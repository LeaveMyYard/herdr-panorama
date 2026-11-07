//! Agent resume through the real writer, over a recorded `hc` world and a fake app.
mod app_feed_support;
use app_feed_support::{ID, Installed};
use serde_json::{Value, json};
use std::{os::unix::fs::PermissionsExt, path::Path, time::Duration};

const SESSION: &str = "11111111-2222-4333-8444-555555555555";
const LOST_HOLDER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CORE: &str = "0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f";
/// A writer can be inside a slow `hc` call under load; it stops after that.
const PATIENCE: Duration = Duration::from_secs(40);
const QUIESCE: Duration = Duration::from_secs(15);

fn now_ms() -> u64 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    now.unwrap().as_millis() as u64
}

fn write(path: &Path, bytes: &[u8], mode: u32) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

/// A real process for the shell: its children are read from `/proc`.
fn shell_pid() -> u32 {
    static PID: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *PID.get_or_init(|| {
        #[allow(clippy::zombie_processes)]
        let child = std::process::Command::new("sleep")
            .arg("300")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        // It outlives the test on purpose: it ends by itself.
        std::mem::forget(child);
        pid
    })
}

fn world() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/hc-resume-recovered.json"
    );
    let mut world: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    world["get"]["pid"] = json!(shell_pid());
    world["get"]["foreground"]["pid"] = json!(shell_pid());
    world
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
    write(&install.bin.join("hc.py"), hc.as_bytes(), 0o700);
    // The typing probe has a short deadline; on a busy machine Python can't start inside it.
    let shim = format!(
        "#!/bin/sh\nif [ \"$1\" = connect ]; then read -r _; echo '{{\"id\":1,\"result\":{{}}}}'; exit 0; fi\nif [ \"$1\" = attach ]; then printf '\\033[?2004h\\033[1;3H'; exec sleep 30; fi\nexec {:?} \"$@\"\n",
        install.bin.join("hc.py")
    );
    write(&install.bin.join("hc"), shim.as_bytes(), 0o700);
}

/// The same world while its shell has not started yet.
fn waiting() -> Value {
    let mut lost = world();
    lost["get"]["pid"] = Value::Null;
    lost["get"]["foreground"] = Value::Null;
    lost["get"]["reason"] = json!("stopped_by_restart");
    lost["read"]["text"] = json!("");
    lost
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
        "launch":"plain","dropped":false,"no_daemon":false,"config":"default",
        "proved":true,"ending_ms":null});
    let saved = json!({"v":1,"install_id":ID,"core":CORE,"holders":[holder],"closed":closed,
        "boot":"an-earlier-boot","seen_ms":now_ms(),
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
    install.wait_feed_within(PATIENCE, |v| v["v"] == 2 && !resume_path(&install).exists());
    std::thread::sleep(Duration::from_millis(600));
    let seen = calls(&install);
    assert!(!seen.is_empty());
    for call in &seen {
        let verb = call["args"][0].as_str().unwrap();
        assert!(matches!(verb, "ls" | "events"), "{call}");
    }
    assert_eq!(install.feed().get("resume"), None);
    let _ = install.quiesce_within(QUIESCE).unwrap();
}

// Other systems have no boot id, so there a loss always waits for a click.
#[cfg(target_os = "linux")]
#[test]
fn a_recorded_loss_is_typed_once_through_the_real_writer() {
    let install = Installed::new();
    serve(&install, &waiting());
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
    install.wait_feed_within(PATIENCE, |v| state(v) == "waiting");
    serve(&install, &world());
    install.wait_feed_within(PATIENCE, |v| state(v) == "sent");
    let sent = sends(&install);
    assert_eq!(sent.len(), 1);
    let typed = format!("env -u CLAUDE_CONFIG_DIR claude --resume {SESSION}");
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
    let (_home, _r, _lock) = install.quiesce_within(QUIESCE).unwrap();
}

#[test]
fn a_stop_without_a_reboot_waits_for_two_clicks_and_types_once() {
    let install = Installed::new();
    serve(&install, &waiting());
    toggle_on(&install);
    seed(&install, LOST_HOLDER, false);
    let path = resume_path(&install);
    let mut saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    saved["boot"] = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .map_or(Value::Null, |id| json!(id.trim()));
    write(&path, &serde_json::to_vec(&saved).unwrap(), 0o600);
    assert!(install.ensure().status.success());
    install.wait_feed_within(PATIENCE, |v| state(v) == "waiting");
    serve(&install, &world());
    install.wait_feed_within(PATIENCE, |v| state(v) == "skipped");
    std::thread::sleep(Duration::from_millis(2500));
    assert!(sends(&install).is_empty(), "nobody asked");
    let entry = install.feed()["resume"]["loss"]["entries"][0].clone();
    assert_eq!(entry["click"], true);
    let id = install.feed()["resume"]["loss"]["id"].as_u64().unwrap();
    let press = |token: Value| {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
        let at = now.unwrap().as_millis() as u64;
        let clicks = json!({"t5": {"loss": id, "token": token, "at": at}}).to_string();
        let store = json!({"agents.resume": "\"on\"", "agents.resume.clicks": clicks}).to_string();
        write(
            &install.app.join("data/panorama-spaces/store.json"),
            store.as_bytes(),
            0o600,
        );
    };
    press(Value::Null);
    install.wait_feed_within(PATIENCE, |v| {
        v["resume"]["loss"]["entries"][0]["look"].is_object()
    });
    assert!(sends(&install).is_empty(), "the first click only looks");
    let view = install.feed()["resume"]["loss"]["entries"][0]["look"].clone();
    assert_eq!(view["lines"], json!(["$"]));
    press(view["token"].clone());
    install.wait_feed_within(PATIENCE, |v| state(v) == "sent");
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(sends(&install).len(), 1);
    let _ = install.quiesce_within(QUIESCE).unwrap();
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
    install.wait_feed_within(PATIENCE, |v| v["resume"]["tracked"] == 1);
    std::thread::sleep(Duration::from_millis(2500));
    assert!(sends(&install).is_empty());
    assert_eq!(install.feed()["resume"]["loss"], Value::Null);
    let _ = install.quiesce_within(QUIESCE).unwrap();
}

#[test]
fn a_loss_after_a_clean_close_is_shown_with_its_command_and_not_typed() {
    let install = Installed::new();
    serve(&install, &world());
    toggle_on(&install);
    seed(&install, LOST_HOLDER, true);
    assert!(install.ensure().status.success());
    install.wait_feed_within(PATIENCE, |v| state(v) == "skipped");
    std::thread::sleep(Duration::from_millis(2500));
    assert!(sends(&install).is_empty());
    let entry = install.feed()["resume"]["loss"]["entries"][0].clone();
    assert_eq!(
        entry["command"],
        format!("env -u CLAUDE_CONFIG_DIR claude --resume {SESSION}")
    );
    assert!(entry["reason"].as_str().unwrap().contains("closed"));
    let _ = install.quiesce_within(QUIESCE).unwrap();
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
    install.wait_feed_within(PATIENCE, |v| v["resume"]["tracked"] == 1);
    let (_home, _r, _lock) = install.quiesce_within(QUIESCE).unwrap();
    let saved: Value =
        serde_json::from_slice(&std::fs::read(resume_path(&install)).unwrap()).unwrap();
    assert_eq!(saved["closed"], true);
}

#[test]
fn a_power_cut_in_any_staging_write_does_not_stop_the_writer_for_good() {
    for name in [".ownership.tmp", ".start.tmp", ".control.tmp"] {
        let install = Installed::new();
        serve(&install, &world());
        let staging = install
            .app
            .join(format!("data/panorama/runtime/{ID}/{name}"));
        // What a cut between the staging write and the rename leaves: partial bytes.
        write(&staging, b"{\"v\":1,\"install", 0o600);
        assert!(install.ensure().status.success(), "{name}");
        install.wait_feed_within(PATIENCE, |v| v["v"] == 2);
        assert!(!staging.exists(), "{name}");
        let _ = install.quiesce_within(QUIESCE).unwrap();
    }
}
