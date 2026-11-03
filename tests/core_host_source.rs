//! Core-host enrichment over a real owned test process and recorded fake hc JSON.
mod codex_support;
mod collect_support;

use collect_support::{S1, STALE, Scenario, WINDOW};
use herdr_agent_kit::{agent_procs::start_token, plugin_env::PluginEnv};
use panorama::host::{Core, SnapshotSource, source::CoreSource};
use serde_json::json;
use std::{
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Child, Command},
    time::Duration,
};

struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A dedicated executable keeps its harness name without relying on renamed multicall tools.
fn owned_agent(dir: &Path, harness: &str) -> Owned {
    let source = dir.join("owned_agent.rs");
    let executable = dir.join(harness);
    std::fs::write(
        &source,
        "fn main() { std::thread::sleep(std::time::Duration::from_secs(30)); }\n",
    )
    .unwrap();
    let compiled = Command::new("rustc")
        .arg("--edition=2024")
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "owned-process fixture compilation failed: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    Owned(Command::new(executable).spawn().unwrap())
}

/// This fixture must not depend on which Python `/usr/bin/env` finds after env_clear.
fn native_host_commands(dir: &Path) {
    let source = dir.join("host_fixture.rs");
    std::fs::write(
        &source,
        r##"use std::{fs,io::Write,process};
fn main() {
    let exe=std::env::current_exe().unwrap();
    let root=exe.parent().unwrap();
    let args:Vec<_>=std::env::args().skip(1).collect();
    if exe.file_name().unwrap()=="herdr" {
        fs::write(root.join("tui-called"),args.join(" ")).unwrap();
        process::exit(7);
    }
    writeln!(fs::OpenOptions::new().create(true).append(true).open(root.join("hc-called")).unwrap(),"{}",args.join(" ")).unwrap();
    if root.join("hc-fail").exists() {
        eprintln!("fixture hc intentional failure");
        process::exit(7);
    }
    let args:Vec<_>=args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["ls","--json"]=>println!("{}",r#"{"hidden_finished":0,"terminals":[{"address":"t2","agent":"claude-code","cwd":"/fixture","state":"live","status":"blocked","turn":0,"where":"untrusted / space"}]}"#),
        ["get","--json","t2"]=>println!("{}",r#"{"address":"t2","state":"live","foreground":null}"#),
        _=>process::exit(9),
    }
}
"##,
    )
    .unwrap();
    let compiled = Command::new("rustc")
        .args(["--edition=2024", "--crate-name=core_host_fixture"])
        .arg(source)
        .arg("-o")
        .arg(dir.join("hc"))
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "native host fixture compilation failed: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    std::fs::copy(dir.join("hc"), dir.join("herdr")).unwrap();
}

#[test]
fn exact_terminal_hook_identity_enriches_and_pid_reuse_or_legacy_records_do_not() {
    let sc = Scenario::new("demo");
    let mut owned = owned_agent(sc.root(), "claude");
    let pid = owned.0.id();
    let start = start_token(pid).unwrap();
    let hooks = sc.root().join("core-hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join(format!("{S1}.jsonl"));
    let record = json!({"v":1,"event":"session","host":"herdr-core","harness":"claude","terminal":"t2","pid":pid,"pstart":start});
    std::fs::write(&hook, record.to_string() + "\n").unwrap();
    let binary = sc.root().join("hc");
    std::fs::write(&binary,format!(r#"#!/usr/bin/env python3
import json,sys
assert sys.argv[1:] in [['ls','--json'],['get','--json','t2']]
if sys.argv[1]=='ls': value={{'hidden_finished':0,'terminals':[{{'address':'t2','agent':'claude-code','cwd':'/fixture/shared','state':'live','status':'working','turn':1,'where':'untrusted / label'}}]}}
else: value={{'address':'t2','state':'live','foreground':{{'pid':{pid},'name':'claude','argv':['claude'],'cwd':'/fixture/shared'}}}}
print(json.dumps(value))
"#)).unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let env = PluginEnv::from_vars(Vec::<(String, String)>::new(), Some(sc.root().into()), 1000);
    let mut source = CoreSource {
        core: Core::new(binary.into_os_string(), Duration::from_secs(1)),
        env,
        hooks_dir: hooks,
    };
    let mut collector = sc.collector().with_core_source();
    assert!(panorama::host::records::has_live_core(
        &source.hooks_dir,
        &source.core
    ));
    let observation = source.read();
    assert!(observation.errors.is_empty());
    let swarm = collector.collect(observation.snapshot.as_ref(), None, sc.now(), WINDOW, STALE);
    assert_eq!(swarm.orchestrators[0].session_id.as_deref(), Some(S1));
    assert!(!swarm.orchestrators[0].threads.is_empty());
    assert_eq!(swarm.orchestrators[0].pane_id, "core:t2");
    assert!(swarm.orchestrators[0].tab_id.is_none());
    // Same cwd and existing transcripts do not substitute for an exact address record.
    let mut wrong = record.clone();
    wrong["terminal"] = json!("t3");
    std::fs::write(&hook, wrong.to_string()).unwrap();
    let observation = source.read();
    let swarm = collector.collect(observation.snapshot.as_ref(), None, sc.now(), WINDOW, STALE);
    assert!(swarm.orchestrators[0].session_id.is_none());
    assert!(swarm.orchestrators[0].threads.is_empty());
    let mut legacy = record.clone();
    legacy.as_object_mut().unwrap().remove("terminal");
    std::fs::write(&hook, legacy.to_string()).unwrap();
    assert!(!source.read().errors.is_empty());
    std::fs::write(&hook, record.to_string()).unwrap();
    owned.0.kill().unwrap();
    owned.0.wait().unwrap();
    assert!(!panorama::host::records::has_live_core(
        &source.hooks_dir,
        &source.core
    ));
    let observation = source.read();
    assert!(!observation.errors.is_empty());
    assert!(
        observation.snapshot.unwrap()["agents"][0]
            .get("agent_session")
            .is_none()
    );
}

#[test]
fn snapshot_cli_uses_core_before_tui_and_source_failures_are_data() {
    let dir = tempfile::tempdir().unwrap();
    native_host_commands(dir.path());
    let hc = dir.path().join("hc");
    let herdr = dir.path().join("herdr");
    // Verify that executing the fake TUI really leaves the marker used by the no-TUI guard.
    let probe = Command::new(&herdr).env_clear().output().unwrap();
    assert_eq!(probe.status.code(), Some(7));
    assert!(dir.path().join("tui-called").exists());
    std::fs::remove_file(dir.path().join("tui-called")).unwrap();
    for success in [true, false] {
        if success {
            assert!(!dir.path().join("hc-fail").exists());
        } else {
            std::fs::write(dir.path().join("hc-fail"), b"intentional failure").unwrap();
        }
        let probe = Command::new(&hc)
            .env_clear()
            .args(["ls", "--json"])
            .output()
            .unwrap();
        assert_eq!(probe.status.success(), success);
        if success {
            let listed: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
            assert_eq!(listed["terminals"][0]["address"], "t2");
        } else {
            assert_eq!(probe.status.code(), Some(7));
            assert!(String::from_utf8_lossy(&probe.stderr).contains("intentional failure"));
        }
        std::fs::remove_file(dir.path().join("hc-called")).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_panorama"))
            .env_clear()
            .args(["snapshot", "--json"])
            .env("PATH", format!("{}:/usr/bin:/bin", dir.path().display()))
            .env("HOME", dir.path())
            .env("XDG_STATE_HOME", dir.path().join("state"))
            .env("HERDR_PLUGIN_CONFIG_DIR", dir.path().join("config"))
            .env("HERDR_INBOX_DB", dir.path().join("no-inbox"))
            .env("CLAUDE_CONFIG_DIR", dir.path().join("claude"))
            .env("HC_TERMINAL_ID", "t2")
            .env("HERDR_PANE_ID", "w1:p1")
            .env("HERDR_BIN_PATH", &herdr)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let feed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let calls = std::fs::read_to_string(dir.path().join("hc-called")).unwrap_or_default();
        assert_eq!(
            feed["herdr_ok"],
            success,
            "success={success}; model={feed}; stderr={}; hc calls={calls}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(calls.lines().next(), Some("ls --json"));
        assert!(!feed["errors"].as_array().unwrap().is_empty());
        assert_eq!(
            feed["orchestrators"].as_array().unwrap().len(),
            usize::from(success)
        );
        if success {
            assert!(calls.lines().any(|line| line == "get --json t2"));
            assert_eq!(feed["orchestrators"][0]["pane_id"], "core:t2");
            assert_eq!(feed["orchestrators"][0]["herdr_status"], "blocked");
            assert!(feed["orchestrators"][0]["session_id"].is_null());
        } else {
            assert!(
                feed["errors"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|error| { error.as_str() == Some("herdr-core: hc read failed") })
            );
        }
        assert!(!dir.path().join("tui-called").exists());
    }
}

#[test]
fn core_codex_binding_and_one_shot_snapshot_keep_existing_history_warmup() {
    let fixture = codex_support::Fixture::new("complete");
    let process_dir = fixture.root().join("owned");
    std::fs::create_dir(&process_dir).unwrap();
    let owned = owned_agent(&process_dir, "codex");
    let pid = owned.0.id();
    let hc = fixture.root().join("hc");
    std::fs::write(&hc, format!(r#"#!/usr/bin/env python3
import json,sys
assert sys.argv[1:] in [['ls','--json'],['get','--json','t2']]
if sys.argv[1]=='ls': value={{'hidden_finished':0,'terminals':[{{'address':'t2','agent':'codex','cwd':'/same/cwd','state':'live','status':'working','turn':1,'where':'untrusted / label'}}]}}
else: value={{'address':'t2','state':'live','foreground':{{'pid':{pid},'name':'codex','argv':['codex'],'cwd':'/same/cwd'}}}}
print(json.dumps(value))
"#)).unwrap();
    std::fs::set_permissions(&hc, std::fs::Permissions::from_mode(0o700)).unwrap();
    let no_tui = fixture.root().join("no-tui.sock");
    let binding = fixture
        .binary(&no_tui)
        .args(["codex-bind", codex_support::ROOT_A, "--terminal", "t2"])
        .output()
        .unwrap();
    assert!(
        binding.status.success(),
        "{}",
        String::from_utf8_lossy(&binding.stderr)
    );
    let out = fixture
        .binary(&no_tui)
        .env("HC_TERMINAL_ID", "t2")
        .args(["snapshot", "--json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let model: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let root = &model["orchestrators"][0];
    assert_eq!(root["session_id"], codex_support::ROOT_A);
    assert!(!root["threads"].as_array().unwrap().is_empty());
    assert_eq!(
        root["threads"][0]["id"],
        format!("codex:{}", codex_support::ROOT_A)
    );
}
