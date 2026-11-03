use super::super::tests::{fixture, processes, state_env};
use super::*;
use serde_json::json;
use std::{os::unix::fs::PermissionsExt, time::Duration};

const ROOT: &str = "11111111-1111-4111-8111-111111111111";

fn core(dir: &std::path::Path, change: bool) -> Core {
    let path = dir.join("hc");
    std::fs::write(&path,format!(r#"#!/usr/bin/env python3
import json,sys,pathlib
assert sys.argv[1:] == ['get','--json','t2']
count=pathlib.Path(__file__).with_name('hc-count')
n=int(count.read_text()) if count.exists() else 0
count.write_text(str(n+1))
print(json.dumps({{'address':'t2','state':'live','foreground':{{'pid':99 if {change} and n else 20,'name':'codex','argv':['codex','--no-daemon'],'cwd':'/fixture'}}}}))
"#,change=if change {"True"} else {"False"})).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    Core::new(path.into_os_string(), Duration::from_secs(1))
}

fn request() -> super::super::bind::Request<'static> {
    super::super::bind::Request {
        pane: "core:t2",
        id: ROOT,
        automatic: true,
        owner: Some(Owner {
            pid: 20,
            start: "start".into(),
        }),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn core_binding_reuses_root_proof_and_cannot_overwrite_tui_namespace() {
    let (dir, options) = fixture(
        json!({"id":ROOT,"parentThreadId":null,"source":"cli"}),
        false,
    );
    let env = state_env(dir.path());
    let hc = core(dir.path(), false);
    let tui = Store::lock(&env, "core:t2").unwrap();
    let old = Association {
        version: 1,
        pane_id: "core:t2".into(),
        thread_id: ROOT.into(),
        owner_pid: 20,
        owner_start: "start".into(),
        embedded: true,
        conflicted: false,
        host: None,
        terminal: None,
        pidns: None,
    };
    tui.write(&old).unwrap();
    bind(&hc, &env, &options, "t2", request(), &processes)
        .await
        .unwrap();
    assert!(tui.existing("core:t2").unwrap().unwrap().host.is_none());
    let info = hc.process("t2").unwrap();
    assert_eq!(
        resolve(&env, "t2", &info, processes().as_ref()).as_deref(),
        Some(ROOT)
    );
    assert!(resolve(&env, "t3", &info, processes().as_ref()).is_none());
    assert!(resolve(&env, "vps/t2", &info, processes().as_ref()).is_none());
    let mut conflict = request();
    conflict.id = "22222222-2222-4222-8222-222222222222";
    assert!(
        bind(&hc, &env, &options, "t2", conflict, &processes)
            .await
            .is_err()
    );
    assert!(resolve(&env, "t2", &info, processes().as_ref()).is_none());
    let mut recovery = request();
    recovery.automatic = false;
    recovery.owner = None;
    bind(&hc, &env, &options, "t2", recovery, &processes)
        .await
        .unwrap();
    assert_eq!(
        resolve(&env, "t2", &info, processes().as_ref()).as_deref(),
        Some(ROOT)
    );
    let mut reused = herdr_agent_kit::agent_procs::fake::FakeProcesses::default();
    reused.insert(20, 1, "codex", "new-start", &["codex", "--no-daemon"]);
    assert!(resolve(&env, "t2", &info, &reused).is_none());
}

#[test]
fn foreign_core_namespace_is_unavailable_even_with_matching_pid_and_start() {
    let dir = tempfile::tempdir().unwrap();
    let env = state_env(dir.path());
    let store = Store::core_lock(&env, "t2").unwrap();
    let mut foreign = Association {
        version: 1,
        pane_id: "core:t2".into(),
        thread_id: ROOT.into(),
        owner_pid: 20,
        owner_start: "start".into(),
        embedded: true,
        conflicted: false,
        host: Some("herdr-core".into()),
        terminal: Some("t2".into()),
        pidns: None,
    };
    let info = json!({"process_info":{"pane_id":"core:t2","foreground_processes":[{"pid":20}]}});
    store.write(&foreign).unwrap();
    assert_eq!(
        resolve(&env, "t2", &info, processes().as_ref()).as_deref(),
        Some(ROOT)
    );
    foreign.pidns = Some("pid:[foreign-namespace]".into());
    store.write(&foreign).unwrap();
    assert!(resolve(&env, "t2", &info, processes().as_ref()).is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn child_metadata_or_foreground_switch_cannot_publish_a_binding() {
    for (parent, change) in [(json!(ROOT), false), (Value::Null, true)] {
        let (dir, options) = fixture(
            json!({"id":ROOT,"parentThreadId":parent,"source":"cli"}),
            false,
        );
        let env = state_env(dir.path());
        let hc = core(dir.path(), change);
        assert!(
            bind(&hc, &env, &options, "t2", request(), &processes)
                .await
                .is_err()
        );
        let info =
            json!({"process_info":{"pane_id":"core:t2","foreground_processes":[{"pid":20}]}});
        assert!(resolve(&env, "t2", &info, processes().as_ref()).is_none());
    }
}
