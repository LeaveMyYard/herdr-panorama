use super::*;
use crate::machines::Machine;
use herdr_agent_kit::herdr_api::fake::{CliReply, CliStub, FakeHerdr};
use std::time::{Duration, Instant};

fn env(dir: &std::path::Path, config: &str) -> PluginEnv {
    std::fs::create_dir_all(dir.join("cfg")).unwrap();
    std::fs::write(dir.join("cfg/config.toml"), config).unwrap();
    PluginEnv::from_vars(
        [
            ("HERDR_PLUGIN_CONFIG_DIR", dir.join("cfg")),
            ("HERDR_PLUGIN_STATE_DIR", dir.join("state")),
        ],
        Some(dir.to_path_buf()),
        1000,
    )
}

#[test]
fn no_saved_machine_starts_no_watcher() {
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeHerdr::start();
    let stub = CliStub::recorded(); // captured: `machine list --json` is []
    let r = start(
        &env(dir.path(), ""),
        fake.blocking_client().with_bin(stub.bin()),
    );
    assert!(r.watchers.is_empty());
    assert_eq!(stub.calls(), [["machine", "list", "--json"]]);
}

#[test]
fn a_saved_machine_gets_a_watcher_that_reports_its_failure() {
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeHerdr::start();
    let mut stub = CliStub::recorded(); // `--machine nosuch api snapshot`: exit 2 (captured)
    let rows = r#"[{"id": "nosuch", "label": "box", "target": "box", "session": "default", "enabled": true}]"#;
    stub.on(&["machine", "list", "--json"], CliReply::ok(rows));
    let mut r = start(
        &env(dir.path(), ""),
        fake.blocking_client().with_bin(stub.bin()),
    );
    assert_eq!(r.watchers.len(), 1);
    let until = Instant::now() + Duration::from_secs(5);
    while !r.watchers[0].outcome().tried && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut local = panorama_core::model::Swarm::default();
    r.merge(&mut local);
    assert_eq!(
        local.errors,
        ["box: unreachable (error: unknown machine 'nosuch'; use `herdr machine list`)"]
    );
    r.stop();
    for w in r.watchers.drain(..) {
        w.join(); // no watcher thread outlives the test
    }
    // the snapshot failed before any sync: nothing was mirrored
    assert!(!dir.path().join("state/remote").exists());
}

#[test]
fn machines_none_asks_herdr_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeHerdr::start();
    let stub = CliStub::recorded();
    let r = start(
        &env(dir.path(), "machines = \"none\"\n"),
        fake.blocking_client().with_bin(stub.bin()),
    );
    assert!(r.watchers.is_empty() && stub.calls().is_empty());
}

#[test]
fn a_machine_s_collector_reads_its_mirror_without_process_checks_and_windows_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let e = env(dir.path(), "done_window = \"2h\"\nstale_after = \"1h\"\n");
    let o = mirror_options(
        &e,
        &dir.path().join("state"),
        &Machine::new("ab-1", "box", "box"),
    );
    assert!(!o.procs);
    assert_eq!(o.projects, dir.path().join("state/remote/ab1/projects"));
    let fake = FakeHerdr::start();
    let mut w = windows(e, fake.blocking_client());
    assert_eq!(w(), (7200.0, 3600.0)); // (done window, stale after)
}
