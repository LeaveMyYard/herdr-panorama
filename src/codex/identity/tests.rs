use super::*;
use herdr_agent_kit::{
    agent_procs::fake::FakeProcesses,
    herdr_api::fake::{FakeHerdr, Reply},
};
use serde_json::{Value, json};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    time::Duration,
};

const ROOT: &str = "11111111-1111-4111-8111-111111111111";
const CHILD: &str = "22222222-2222-4222-8222-222222222222";

pub(super) fn fixture(thread: Value, delay: bool) -> (tempfile::TempDir, crate::codex::Options) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("codex");
    std::fs::write(dir.path().join("thread.json"), thread.to_string()).unwrap();
    std::fs::write(
        &path,
        format!(
            r#"#!/usr/bin/env python3
import json,sys,pathlib,os,time
home=pathlib.Path(__file__).parent
(home/'pid').write_text(str(os.getpid()))
for line in sys.stdin:
    request=json.loads(line)
    with (home/'calls').open('a') as out: out.write(json.dumps(request)+'\n')
    if 'id' not in request: continue
    if request['method']=='initialize': result={{}}
    elif request['method']=='thread/read':
        if (home/'pause').exists():
            (home/'reading').write_text('ready')
            while not (home/'release').exists(): time.sleep(0.002)
        if {delay}: time.sleep(30)
        result={{'thread':json.loads((home/'thread.json').read_text())}}
    else: raise RuntimeError('forbidden operation')
    print(json.dumps({{'id':request['id'],'result':result}}),flush=True)
"#,
            delay = if delay { "True" } else { "False" }
        ),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let opts = crate::codex::Options {
        executable: path,
        codex_home: dir.path().into(),
        timeout: Duration::from_secs(5),
        refresh_interval: Duration::from_secs(5),
    };
    (dir, opts)
}
fn thread() -> Value {
    json!({"id":ROOT,"parentThreadId":null,"source":"cli"})
}
fn fake_pane(fake: &FakeHerdr, pane: &str, agent: &str) {
    fake.on(
        "pane.get",
        Reply::Result(json!({"type":"pane_info","pane":{"pane_id":pane,"agent":agent}})),
    );
    fake.on("pane.process_info", Reply::Result(process_info(pane, 20)));
}

#[test]
fn only_exact_root_start_events_are_candidates() {
    for source in ["startup", "resume", "fork"] {
        let value = json!({"hook_event_name":"SessionStart","session_id":ROOT,"source":source});
        assert_eq!(cli::candidate(&value), Some((ROOT, source)));
    }
    for value in [
        json!({}),
        json!({"hook_event_name":"SubagentStart","session_id":ROOT,"agent_id":CHILD,"source":"startup"}),
        json!({"hook_event_name":"SessionStart","session_id":ROOT,"source":"compact"}),
        json!({"hook_event_name":"SessionStart","session_id":ROOT,"source":"clear"}),
        json!({"hook_event_name":"SessionStart","session_id":ROOT,"source":"startup","agent_id":CHILD}),
        json!({"hook_event_name":"SessionStart","session_id":"bad","source":"startup"}),
    ] {
        assert_eq!(cli::candidate(&value), None);
    }
}
#[test]
fn root_proof_refuses_missing_ancestry_mismatch_and_subagent_source() {
    assert!(root::is_root(&json!({"thread":thread()}), ROOT));
    for thread in [
        json!({"id":ROOT,"source":"cli"}),
        json!({"id":CHILD,"parentThreadId":null,"source":"cli"}),
        json!({"id":ROOT,"parentThreadId":CHILD,"source":"cli"}),
        json!({"id":ROOT,"parentThreadId":null,"source":{"subAgent":{"threadSpawn":{}}}}),
        json!({"id":ROOT,"parentThreadId":null}),
    ] {
        assert!(!root::is_root(&json!({"thread":thread}), ROOT));
    }
}
#[tokio::test(flavor = "current_thread")]
async fn explicit_binding_validates_root_and_exact_pane_without_native_writes() {
    let (dir, options) = fixture(thread(), false);
    let fake = FakeHerdr::start();
    fake_pane(&fake, "w1:p1", "codex");
    let env = state_env(dir.path());
    let store = store::Store::lock(&env, "w1:p1").unwrap();
    bind::bind(
        &fake.client(),
        &options,
        request(ROOT, false),
        &store,
        &processes,
    )
    .await
    .unwrap();
    assert_eq!(
        store::resolve_with(
            &env,
            "w1:p1",
            None,
            || Some(process_info("w1:p1", 20)),
            processes().as_ref()
        ),
        Some(ROOT.into())
    );
    assert!(fake.params_of("pane.report_agent_session").is_empty());
    let calls: Vec<Value> = std::fs::read_to_string(dir.path().join("calls"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        calls
            .iter()
            .map(|c| c["method"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["initialize", "initialized", "thread/read"]
    );
    assert_eq!(
        calls[2]["params"],
        json!({"threadId":ROOT,"includeTurns":false})
    );
}
#[tokio::test(flavor = "current_thread")]
async fn wrong_pane_agent_child_or_unavailable_metadata_never_registers() {
    for (pane, agent, thread) in [
        ("w1:p2", "codex", thread()),
        ("w1:p1", "claude", thread()),
        (
            "w1:p1",
            "codex",
            json!({"id":ROOT,"parentThreadId":CHILD,"source":"cli"}),
        ),
        ("w1:p1", "codex", json!({"id":ROOT,"source":"cli"})),
    ] {
        let (dir, options) = fixture(thread, false);
        let fake = FakeHerdr::start();
        fake_pane(&fake, pane, agent);
        let env = state_env(dir.path());
        let store = store::Store::lock(&env, "w1:p1").unwrap();
        assert!(
            bind::bind(
                &fake.client(),
                &options,
                request(ROOT, true),
                &store,
                &processes
            )
            .await
            .is_err()
        );
        assert!(
            store::resolve_with(
                &env,
                "w1:p1",
                None,
                || Some(process_info("w1:p1", 20)),
                processes().as_ref()
            )
            .is_none()
        );
        assert!(fake.params_of("pane.report_agent_session").is_empty());
    }
}
#[tokio::test(flavor = "current_thread")]
async fn metadata_timeout_reaps_only_its_owned_child() {
    let (dir, mut options) = fixture(thread(), false);
    // A native shell peer makes readiness independent of cold Python startup.
    // Its exact PID is published before the bounded RPC deliberately stalls.
    std::fs::write(
        &options.executable,
        r#"#!/bin/sh
fixture_dir=${0%/*}
printf '%s' "$$" > "$fixture_dir/pid"
while IFS= read -r line; do
    case "$line" in
        *'"method":"initialize"'*) printf '%s\n' '{"id":1,"result":{}}' ;;
        *'"method":"thread/read"'*)
            printf '%s' ready > "$fixture_dir/ready"
            IFS= read -r blocked
            exit 1 ;;
    esac
done
"#,
    )
    .unwrap();
    options.timeout = Duration::from_secs(1);
    let mut unrelated = TestChild(
        std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .unwrap(),
    );
    let ready = async {
        tokio::time::timeout(Duration::from_secs(2), async {
            while !dir.path().join("ready").exists() {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
            let raw = std::fs::read_to_string(dir.path().join("pid"))
                .unwrap()
                .parse::<i32>()
                .unwrap();
            let pid = rustix::process::Pid::from_raw(raw).unwrap();
            assert!(
                rustix::process::test_kill_process(pid).is_ok(),
                "owned peer must be alive and ready before timeout"
            );
            pid
        })
        .await
        .expect("owned metadata peer reached readiness")
    };
    let before = std::time::Instant::now();
    let verify = tokio::time::timeout(Duration::from_secs(5), root::verify(&options, ROOT));
    let (result, pid) = tokio::join!(verify, ready);
    assert_eq!(
        result.expect("identity deadline must fire before outer test bound"),
        Err("Codex metadata read timed out")
    );
    assert!(before.elapsed() < Duration::from_secs(3));
    assert_eq!(
        rustix::process::test_kill_process(pid),
        Err(rustix::io::Errno::SRCH),
        "owned child must be killed and reaped on every platform"
    );
    assert!(unrelated.0.try_wait().unwrap().is_none());
    unrelated.0.kill().unwrap();
    unrelated.0.wait().unwrap();
}

#[test]
fn embedded_ancestry_requires_unambiguous_flag_and_start_identity() {
    let mut procs = FakeProcesses::default();
    procs.insert(
        30,
        20,
        "sh",
        "shell",
        &["/bin/sh", "-c", "panorama codex-hook"],
    );
    procs.insert(20, 10, "codex", "start", &["/bin/codex", "--no-daemon"]);
    assert_eq!(
        owner::ancestor(&procs, 30),
        Some(owner::Owner {
            pid: 20,
            start: "start".into()
        })
    );
    procs.forget_start(20);
    assert_eq!(owner::ancestor(&procs, 30), None);
    for args in [
        vec!["codex"],
        vec!["codex", "app-server"],
        vec!["codex", "--", "--no-daemon"],
        vec!["codex", "--no-daemon", "daemon"],
    ] {
        procs.insert(20, 10, "codex", "start", &args);
        assert_eq!(owner::ancestor(&procs, 30), None);
    }
}
#[tokio::test(flavor = "current_thread")]
async fn stale_daemon_pane_and_reused_pid_cannot_pass_ownership() {
    use herdr_agent_kit::herdr_api::PaneId;
    let fake = FakeHerdr::start();
    let mut procs = FakeProcesses::default();
    procs.insert(20, 10, "codex", "start", &["codex", "--no-daemon"]);
    let owner = owner::Owner {
        pid: 20,
        start: "start".into(),
    };
    let pane = PaneId::new("w1:p1");
    for (returned, pid, success) in [
        ("w1:p1", 20, true),
        ("w1:p2", 20, false),
        ("w1:p1", 99, false),
    ] {
        fake.on("pane.process_info",Reply::Result(json!({"type":"pane_process_info","process_info":{"pane_id":returned,"foreground_processes":[{"pid":pid}]}})));
        assert_eq!(
            owner::check(&fake.client(), &pane, &owner, &procs)
                .await
                .is_ok(),
            success
        );
    }
    fake.on(
        "pane.process_info",
        Reply::Result(
            json!({"process_info":{"pane_id":"w1:p1","foreground_processes":[{"pid":20}]}}),
        ),
    );
    procs.insert(
        20,
        10,
        "codex",
        "different-start",
        &["codex", "--no-daemon"],
    );
    assert!(
        owner::check(&fake.client(), &pane, &owner, &procs)
            .await
            .is_err()
    );
}

fn process_info(pane: &str, pid: u32) -> Value {
    json!({"process_info":{"pane_id":pane,"foreground_processes":[{"pid":pid}]}})
}
pub(super) fn state_env(dir: &std::path::Path) -> herdr_agent_kit::plugin_env::PluginEnv {
    herdr_agent_kit::plugin_env::PluginEnv::from_vars(
        [("XDG_STATE_HOME", dir.to_str().unwrap())],
        None,
        std::fs::metadata(dir).unwrap().uid(),
    )
}
pub(super) fn processes() -> Box<dyn herdr_agent_kit::agent_procs::ProcessTable> {
    let mut procs = FakeProcesses::default();
    procs.insert(20, 10, "codex", "start", &["codex", "--no-daemon"]);
    Box::new(procs)
}
fn request(id: &str, automatic: bool) -> bind::Request<'_> {
    bind::Request {
        pane: "w1:p1",
        id,
        automatic,
        owner: if automatic {
            Some(owner::Owner {
                pid: 20,
                start: "start".into(),
            })
        } else {
            None
        },
    }
}

#[tokio::test(flavor = "current_thread")]
async fn resume_repeats_fork_invalidates_and_explicit_binding_recovers_without_native_writes() {
    let (dir, options) = fixture(thread(), false);
    let env = state_env(dir.path());
    let store = store::Store::lock(&env, "w1:p1").unwrap();
    let fake = FakeHerdr::start();
    fake_pane(&fake, "w1:p1", "codex");
    for _ in 0..2 {
        bind::bind(
            &fake.client(),
            &options,
            request(ROOT, true),
            &store,
            &processes,
        )
        .await
        .unwrap();
    }
    let resolve = || {
        store::resolve_with(
            &env,
            "w1:p1",
            None,
            || Some(process_info("w1:p1", 20)),
            processes().as_ref(),
        )
    };
    assert_eq!(resolve(), Some(ROOT.into()));
    std::fs::write(
        dir.path().join("thread.json"),
        json!({"id":CHILD,"parentThreadId":null,"source":"cli"}).to_string(),
    )
    .unwrap();
    assert!(
        bind::bind(
            &fake.client(),
            &options,
            request(CHILD, true),
            &store,
            &processes
        )
        .await
        .is_err()
    );
    assert_eq!(resolve(), None);
    bind::bind(
        &fake.client(),
        &options,
        request(CHILD, false),
        &store,
        &processes,
    )
    .await
    .unwrap();
    assert_eq!(resolve(), Some(CHILD.into()));
    // A delayed old event cannot overwrite the explicitly bound fork, only invalidate.
    std::fs::write(dir.path().join("thread.json"), thread().to_string()).unwrap();
    assert!(
        bind::bind(
            &fake.client(),
            &options,
            request(ROOT, true),
            &store,
            &processes
        )
        .await
        .is_err()
    );
    assert_eq!(resolve(), None);
    assert!(fake.params_of("pane.report_agent_session").is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn native_conflict_is_authoritative_and_cannot_be_overwritten() {
    let (dir, options) = fixture(thread(), false);
    let env = state_env(dir.path());
    let store = store::Store::lock(&env, "w1:p1").unwrap();
    let fake = FakeHerdr::start();
    fake_pane(&fake, "w1:p1", "codex");
    fake.on("pane.get",Reply::Result(json!({"pane":{"pane_id":"w1:p1","agent":"codex","agent_session":{"kind":"id","value":CHILD}}})));
    assert!(
        bind::bind(
            &fake.client(),
            &options,
            request(ROOT, false),
            &store,
            &processes
        )
        .await
        .is_err()
    );
    assert_eq!(
        store::resolve_with(
            &env,
            "w1:p1",
            Some(CHILD),
            || panic!("native needs no process lookup"),
            processes().as_ref()
        ),
        Some(CHILD.into())
    );
    assert_eq!(
        store::resolve_with(
            &env,
            "w1:p1",
            Some("invalid"),
            || panic!("no fallback for invalid native"),
            processes().as_ref()
        ),
        None
    );
    assert!(fake.params_of("pane.report_agent_session").is_empty());
}

#[test]
fn stored_association_is_bounded_and_requires_fresh_exact_foreground_identity() {
    let dir = tempfile::tempdir().unwrap();
    let env = state_env(dir.path());
    let store = store::Store::lock(&env, "w1:p1").unwrap();
    let a = store::Association {
        host: None,
        terminal: None,
        pidns: None,
        version: 1,
        pane_id: "w1:p1".into(),
        thread_id: ROOT.into(),
        owner_pid: 20,
        owner_start: "start".into(),
        embedded: true,
        conflicted: false,
    };
    store.write(&a).unwrap();
    assert!(store::Store::lock(&env, "w1:p1").is_err());
    for info in [
        process_info("w1:p1", 99),
        process_info("w1:p2", 20),
        json!({"process_info":{"pane_id":"w1:p1"}}),
    ] {
        assert_eq!(
            store::resolve_with(&env, "w1:p1", None, || Some(info), processes().as_ref()),
            None
        );
    }
    let mut reused = FakeProcesses::default();
    reused.insert(20, 10, "codex", "new-start", &["codex", "--no-daemon"]);
    assert_eq!(
        store::resolve_with(
            &env,
            "w1:p1",
            None,
            || Some(process_info("w1:p1", 20)),
            &reused
        ),
        None
    );
    let file = dir.path().join("herdr-panorama/codex-bindings/77313a7031");
    for invalid in [b"bad-json".to_vec(), vec![b' '; 4097]] {
        std::fs::write(&file, invalid).unwrap();
        assert_eq!(
            store::resolve_with(
                &env,
                "w1:p1",
                None,
                || panic!("invalid store must not query"),
                processes().as_ref()
            ),
            None
        );
        assert_eq!(
            store::resolve_with(
                &env,
                "w1:p1",
                Some(CHILD),
                || panic!("native wins"),
                processes().as_ref()
            ),
            Some(CHILD.into())
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn failed_explicit_switch_keeps_old_association_unavailable() {
    let (dir, options) = fixture(thread(), false);
    let env = state_env(dir.path());
    let store = store::Store::lock(&env, "w1:p1").unwrap();
    let fake = FakeHerdr::start();
    fake_pane(&fake, "w1:p1", "codex");
    bind::bind(
        &fake.client(),
        &options,
        request(ROOT, false),
        &store,
        &processes,
    )
    .await
    .unwrap();
    // The proposed new ID is not returned by thread/read, so the old binding must
    // remain suppressed rather than recover merely because this attempt failed.
    assert!(
        bind::bind(
            &fake.client(),
            &options,
            request(CHILD, false),
            &store,
            &processes
        )
        .await
        .is_err()
    );
    assert!(
        store::resolve_with(
            &env,
            "w1:p1",
            None,
            || Some(process_info("w1:p1", 20)),
            processes().as_ref()
        )
        .is_none()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn explicit_shared_daemon_mapping_tracks_tui_lifetime_not_daemon_identity() {
    let (dir, options) = fixture(thread(), false);
    let env = state_env(dir.path());
    let store = store::Store::lock(&env, "w1:p1").unwrap();
    let fake = FakeHerdr::start();
    fake_pane(&fake, "w1:p1", "codex");
    let tui = || -> Box<dyn herdr_agent_kit::agent_procs::ProcessTable> {
        let mut p = FakeProcesses::default();
        p.insert(20, 10, "codex", "tui-start", &["codex"]);
        p.insert(90, 1, "codex", "daemon-start", &["codex", "app-server"]);
        Box::new(p)
    };
    assert!(owner::ancestor(tui().as_ref(), 20).is_none());
    bind::bind(&fake.client(), &options, request(ROOT, false), &store, &tui)
        .await
        .unwrap();
    assert_eq!(
        store::resolve_with(
            &env,
            "w1:p1",
            None,
            || Some(process_info("w1:p1", 20)),
            tui().as_ref()
        ),
        Some(ROOT.into())
    );
    assert_eq!(
        store::resolve_with(
            &env,
            "w1:p1",
            None,
            || Some(process_info("w1:p1", 90)),
            tui().as_ref()
        ),
        None
    );
    // Process evidence cannot reveal an in-process thread switch; the operator
    // explicitly rebinds the new exact ID. Never claim this is daemon telemetry.
    std::fs::write(
        dir.path().join("thread.json"),
        json!({"id":CHILD,"parentThreadId":null,"source":"cli"}).to_string(),
    )
    .unwrap();
    bind::bind(
        &fake.client(),
        &options,
        request(CHILD, false),
        &store,
        &tui,
    )
    .await
    .unwrap();
    assert_eq!(
        store::resolve_with(
            &env,
            "w1:p1",
            None,
            || Some(process_info("w1:p1", 20)),
            tui().as_ref()
        ),
        Some(CHILD.into())
    );
}

#[tokio::test(flavor = "current_thread")]
async fn metadata_read_races_revalidate_foreground_start_and_native_identity() {
    use std::{cell::Cell, rc::Rc};
    for transition in ["foreground", "start", "native"] {
        let (dir, mut options) = fixture(thread(), false);
        options.timeout = Duration::from_secs(10);
        std::fs::write(dir.path().join("pause"), "").unwrap();
        let env = state_env(dir.path());
        let store = store::Store::lock(&env, "w1:p1").unwrap();
        let fake = FakeHerdr::start();
        fake_pane(&fake, "w1:p1", "codex");
        let changed = Rc::new(Cell::new(false));
        let table = || -> Box<dyn herdr_agent_kit::agent_procs::ProcessTable> {
            let mut p = FakeProcesses::default();
            p.insert(
                20,
                10,
                "codex",
                if transition == "start" && changed.get() {
                    "reused"
                } else {
                    "start"
                },
                &["codex", "--no-daemon"],
            );
            Box::new(p)
        };
        let client = fake.client();
        let binding = bind::bind(&client, &options, request(ROOT, true), &store, &table);
        let mutate = async {
            tokio::time::timeout(Duration::from_secs(8), async {
                while !dir.path().join("reading").exists() {
                    tokio::time::sleep(Duration::from_millis(2)).await;
                }
            })
            .await
            .expect("metadata read reached deterministic barrier");
            assert_eq!(fake.params_of("pane.get").len(), 1);
            assert_eq!(fake.params_of("pane.process_info").len(), 1);
            match transition {
                "foreground" => {
                    fake.on(
                        "pane.process_info",
                        Reply::Result(process_info("w1:p1", 99)),
                    );
                }
                "native" => {
                    fake.on("pane.get",Reply::Result(json!({"pane":{"pane_id":"w1:p1","agent":"codex","agent_session":{"kind":"id","value":CHILD}}})));
                }
                _ => changed.set(true),
            }
            std::fs::write(dir.path().join("release"), "").unwrap();
        };
        let (result, ()) = tokio::join!(binding, mutate);
        assert!(result.is_err(), "{transition} must fail final guard");
        assert!(store.existing("w1:p1").unwrap().unwrap().conflicted);
        assert_eq!(
            store::resolve_with(
                &env,
                "w1:p1",
                None,
                || panic!("pending state must not resolve"),
                processes().as_ref()
            ),
            None
        );
        assert!(fake.params_of("pane.report_agent_session").is_empty());
    }
}

#[test]
fn held_store_directory_cannot_be_redirected_by_a_path_swap() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let env = state_env(dir.path());
    let store = store::Store::lock(&env, "w1:p1").unwrap();
    let original = dir.path().join("herdr-panorama/codex-bindings");
    let moved = dir.path().join("moved");
    let outside = dir.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::rename(&original, &moved).unwrap();
    symlink(&outside, &original).unwrap();
    let a = store::Association {
        host: None,
        terminal: None,
        pidns: None,
        version: 1,
        pane_id: "w1:p1".into(),
        thread_id: ROOT.into(),
        owner_pid: 20,
        owner_start: "start".into(),
        embedded: true,
        conflicted: false,
    };
    store.write(&a).unwrap();
    assert!(moved.join("77313a7031").exists());
    assert!(!outside.join("77313a7031").exists());
    assert_eq!(
        store::resolve_with(
            &env,
            "w1:p1",
            None,
            || panic!("swapped directory must fail"),
            processes().as_ref()
        ),
        None
    );
}

struct TestChild(std::process::Child);
impl Drop for TestChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
