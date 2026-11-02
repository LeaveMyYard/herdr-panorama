#![allow(dead_code)]

use panorama::codex;
use panorama::collect::{Collector, Options};
use panorama_core::model::Swarm;
use serde_json::{Value, json};
use std::path::Path;
use std::time::{Duration, Instant};

pub struct Fixture {
    pub dir: tempfile::TempDir,
}

impl Fixture {
    pub fn new(mode: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("mode"), mode).unwrap();
        let executable = dir.path().join("codex");
        // One protocol fixture shared with adapter tests. It rejects every RPC
        // other than initialization and read-only thread history operations.
        std::fs::write(
            &executable,
            include_str!("../../src/codex/tests/fixture.py"),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self { dir }
    }

    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    pub fn options(&self) -> codex::Options {
        codex::Options {
            executable: self.root().join("codex"),
            codex_home: self.root().to_owned(),
            timeout: Duration::from_secs(2),
            refresh_interval: Duration::from_secs(60),
        }
    }

    pub fn collector(&self, enabled: bool) -> Collector {
        let collector = Collector::new(Options {
            projects: self.root().join("projects"),
            events_dir: self.root().join("events"),
            hooks_dir: self.root().join("hooks"),
            procs: false,
            pane_pid: Box::new(|_| {
                panic!("synthetic Codex collection must not inspect Claude processes")
            }),
        });
        if enabled {
            collector.with_codex(self.options())
        } else {
            collector
        }
    }

    pub fn calls(&self) -> Vec<Value> {
        std::fs::read_to_string(self.root().join("calls"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}

pub fn snapshot() -> Value {
    json!({
        "tabs": [{"tab_id":"t1","workspace_id":"w1","label":"Mixed"},
                 {"tab_id":"t2","workspace_id":"w1","label":"Codex"}],
        "panes": [{"pane_id":"panel","tab_id":"t2"}],
        "agents": [
            {"pane_id":"claude","tab_id":"t1","agent":"claude","agent_status":"idle","cwd":"/same/cwd"},
            {"pane_id":"codex-a","tab_id":"t2","agent":"codex","agent_status":"working","cwd":"/same/cwd",
             "agent_session":{"kind":"id","value":"root-a"}},
            {"pane_id":"codex-b","tab_id":"t2","agent":"codex","agent_status":"blocked","cwd":"/same/cwd",
             "agent_session":{"kind":"id","value":"root-b"}},
            {"pane_id":"unbound","tab_id":"t2","agent":"codex","agent_status":"idle","cwd":"/same/cwd"}
        ]
    })
}

pub fn collect(collector: &mut Collector, snapshot: &Value) -> Swarm {
    collector.collect(
        Some(snapshot),
        None,
        panorama::snapshot::now(),
        3600.0,
        43200.0,
    )
}

pub fn ready(collector: &mut Collector, snapshot: &Value) -> Swarm {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let swarm = collect(collector, snapshot);
        if !collector.codex_pending(&swarm) {
            return swarm;
        }
        assert!(
            Instant::now() < deadline,
            "synthetic Codex worker did not finish"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
