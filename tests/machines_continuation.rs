//! A remote conversation continued in a new session (#39), end to end: the forwarded
//! snapshot names the old session, its mirrored transcript ends with `continued-in`, so the
//! new session is mirrored too and the orchestrator shows it, in one refresh (Python
//! `test_machines.test_a_remote_continuation_is_mirrored_and_followed_in_one_refresh`). The
//! "machine" is a local directory, mirrored with real rsync.

use herdr_agent_kit::herdr_api::Snapshot;
use panorama::collect::mirror::MirrorCollector;
use panorama::collect::{Collector, Options};
use panorama::machines::mirror::{Rsync, run};
use panorama::machines::sync::{Syncer, mirror_root};
use panorama::machines::{Machine, Outcome, Refresher, SnapshotApi};
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const OLD: &str = "77777777-7777-4777-8777-777777777777";
const NEW: &str = "88888888-8888-4888-8888-888888888888";

struct Forwarded(Snapshot);

impl SnapshotApi for Forwarded {
    fn machine_snapshot(&self, _: &str) -> Result<Snapshot, String> {
        Ok(self.0.clone())
    }
}

/// A sync from a local "remote" dir into the machine's mirror under `state`.
struct LocalSync {
    remote: PathBuf,
    state: PathBuf,
    synced: Mutex<Vec<Vec<String>>>,
}

impl Syncer for LocalSync {
    fn sync(&self, m: &Machine, sessions: &[String]) -> Option<String> {
        let mut sorted = sessions.to_vec();
        sorted.sort();
        self.synced.lock().unwrap().push(sorted);
        let dest = mirror_root(&self.state, m);
        std::fs::create_dir_all(&dest).unwrap();
        Rsync::new(None).mirror(&format!("{}/", self.remote.display()), &dest, sessions, run)
    }
}

#[test]
fn a_remote_continuation_is_mirrored_and_followed_in_one_refresh() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("remote").join("proj");
    std::fs::create_dir_all(&src).unwrap();
    // as captured: continued-in, then bookkeeping records
    std::fs::write(
        src.join(format!("{OLD}.jsonl")),
        format!(
            "{{\"type\":\"ai-title\",\"aiTitle\":\"work\",\"sessionId\":\"{OLD}\"}}\n\
             {{\"type\":\"continued-in\",\"timestamp\":\"2026-10-16T18:41:47.545Z\",\"sessionId\":\"{OLD}\",\"continuedInSessionId\":\"{NEW}\"}}\n\
             {{\"type\":\"last-prompt\",\"lastPrompt\":\"x\",\"sessionId\":\"{OLD}\"}}\n\
             {{\"type\":\"cost-state\",\"sessionId\":\"{OLD}\",\"totalCostUSD\":1.0}}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        src.join(format!("{NEW}.jsonl")),
        format!(
            "{{\"type\":\"ai-title\",\"aiTitle\":\"work, continued\",\"sessionId\":\"{NEW}\"}}\n"
        ),
    )
    .unwrap();
    let snap: Snapshot = serde_json::from_value(json!({
        "tabs": [{"tab_id": "w1:t1", "workspace_id": "w1"}],
        "agents": [{"pane_id": "w1:p1", "tab_id": "w1:t1", "workspace_id": "w1", "agent": "claude",
                    "agent_session": {"kind": "id", "value": OLD}}]
    }))
    .unwrap();
    let machine = Machine::new("abc123", "devbox", "devbox");
    let state = dir.path().join("state");
    let sync = Arc::new(LocalSync {
        remote: dir.path().join("remote"),
        state: state.clone(),
        synced: Mutex::default(),
    });
    let collector = Collector::new(Options {
        projects: mirror_root(&state, &machine),
        events_dir: dir.path().join("events"),
        hooks_dir: dir.path().join("hooks"),
        procs: false,
        pane_pid: Box::new(|_| panic!("never asked for another machine")),
    });
    let mut r = Refresher {
        machine,
        api: Arc::new(Forwarded(snap)),
        syncer: sync.clone(),
        collector: Box::new(MirrorCollector {
            collector,
            windows: Box::new(|| (3600.0, 12.0 * 3600.0)),
            clock: Box::new(|| 1_792_176_200.0),
        }),
    };
    let out = r.refresh(&Outcome::default());
    let o = &out.swarm.unwrap().orchestrators[0];
    assert_eq!(o.session_id.as_deref(), Some(NEW));
    assert_eq!(o.tab_label, "work, continued");
    assert_eq!(o.pane_id, "abc123:w1:p1");
    let both = vec![OLD.to_owned(), NEW.to_owned()];
    assert_eq!(
        *sync.synced.lock().unwrap(),
        [vec![OLD.to_owned()], both.clone()]
    );
    r.refresh(&Outcome::default()); // known now: one sync with both
    let synced = sync.synced.lock().unwrap();
    assert_eq!((synced.len(), &synced[2]), (3, &both));
}
