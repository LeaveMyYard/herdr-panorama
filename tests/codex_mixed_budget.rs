//! Slow earlier Claude enrichment cannot spend the Codex ownership RPC budget.
mod collect_support;
use collect_support::Scenario;
use herdr_agent_kit::{
    agent_procs::{ProcessTable, SystemProcesses},
    herdr_api::fake::{FakeHerdr, Reply},
    plugin_env::PluginEnv,
};
use panorama::collect::{Collector, Options};
use serde_json::json;
use std::{
    os::unix::fs::PermissionsExt,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
struct Foreground(Child);
impl Drop for Foreground {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn a_valid_codex_association_survives_earlier_claude_work_over_100ms() {
    let sc = Scenario::new("background");
    let owner = Foreground(
        Command::new("/bin/bash")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .args(["-c", "exec -a codex sleep 30"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    while SystemProcesses::new()
        .cmdline(owner.0.id())
        .first()
        .map(String::as_str)
        != Some("codex")
    {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let state = sc.root().join("state");
    let bindings = state.join("herdr-panorama/codex-bindings");
    std::fs::create_dir_all(&bindings).unwrap();
    for path in [&state, &bindings] {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let root = "10000000-0000-4000-8000-000000000001";
    let record = bindings.join("636f646578");
    std::fs::write(&record,serde_json::to_vec(&json!({"version":1,"pane_id":"codex","thread_id":root,"owner_pid":owner.0.id(),"owner_start":SystemProcesses::new().start(owner.0.id()).unwrap(),"embedded":false,"conflicted":false})).unwrap()).unwrap();
    std::fs::set_permissions(&record, std::fs::Permissions::from_mode(0o600)).unwrap();
    let env = PluginEnv::from_vars(
        [("XDG_STATE_HOME", state.as_os_str())],
        Some(sc.root().to_owned()),
        rustix::process::getuid().as_raw(),
    );
    let fake = FakeHerdr::start();
    fake.on("pane.process_info",Reply::Result(json!({"process_info":{"pane_id":"codex","foreground_processes":[{"pid":owner.0.id()}]}})));
    let slow = Arc::new(AtomicUsize::new(0));
    let called = slow.clone();
    let mut collector = Collector::new(Options {
        projects: sc.projects(),
        hooks_dir: sc.root().join("hooks"),
        procs: true,
        pane_pid: Box::new(move |_| {
            called.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(150));
            None
        }),
    })
    .with_codex_identity(env, fake.client().blocking());
    let mut snapshot = sc.snapshot().clone();
    snapshot["tabs"]
        .as_array_mut()
        .unwrap()
        .push(json!({"tab_id":"last","workspace_id":"w1","label":"Codex"}));
    snapshot["agents"]
        .as_array_mut()
        .unwrap()
        .push(json!({"pane_id":"codex","tab_id":"last","agent":"codex","agent_status":"working"}));
    let start = Instant::now();
    let swarm = collector.collect(Some(&snapshot), None, sc.now(), 3600.0, 7200.0);
    assert!(start.elapsed() >= Duration::from_millis(150));
    assert_eq!(
        slow.load(Ordering::SeqCst),
        1,
        "real Claude background path must execute first"
    );
    let codex = swarm.roots.iter().find(|o| o.pane_id == "codex").unwrap();
    assert_eq!(codex.session_id.as_deref(), Some(root));
    assert_eq!(
        fake.calls().len(),
        1,
        "only actual ownership RPC consumes budget"
    );
}
