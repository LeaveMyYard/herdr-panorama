use super::*;
use crate::machines::sync::Syncer;
use crate::machines::{
    Collect, Machine, Outcome, Refresher, Selection, SnapshotApi, Source, StopFlag, Watcher,
};
use herdr_agent_kit::herdr_api::{Machine as Saved, Snapshot};
use panorama_core::model::{Agent, Orchestrator, Status, Thread};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};

fn orch(pane: &str, status: &str) -> Orchestrator {
    let thread: Thread =
        serde_json::from_value(json!({"id": "t1", "name": "t", "source": "turn"})).unwrap();
    let agent = Agent {
        id: format!("{pane}-a"),
        status: Status::from_code(status),
        ..Agent::default()
    };
    let mut o: Orchestrator = serde_json::from_value(
        json!({"pane_id": pane, "tab_id": "t", "tab_label": "o", "session_id": "s"}),
    )
    .unwrap();
    o.threads = vec![Thread {
        agents: vec![agent],
        ..thread
    }];
    o
}

fn tracker(dir: &std::path::Path) -> Arc<Tracker> {
    Arc::new(Tracker::new(dir.join("unseen.json"), Box::new(|| true)))
}

struct OneMachine;

impl Source for OneMachine {
    fn selection(&mut self) -> Selection {
        Selection::All
    }
    fn machines(&mut self) -> Vec<Saved> {
        serde_json::from_value(
            json!([{"id": "m1", "label": "box", "target": "box", "enabled": true}]),
        )
        .unwrap()
    }
}

/// Remotes with one idle watcher whose last result is a remote orchestrator; the watcher's
/// stop flag goes to `flags`.
fn remotes(flags: Arc<Mutex<Vec<StopFlag>>>) -> Remotes {
    let spawn = move |m: Machine| {
        let w = Watcher::idle(m);
        let mut o = orch("m1:w1:p1", "run");
        o.machine = Some("box".into());
        o.machine_id = Some("m1".into());
        let swarm = Swarm {
            orchestrators: vec![o],
            ..Swarm::default()
        };
        w.set_outcome(Outcome {
            swarm: Some(swarm),
            at: Some(Instant::now()),
            error: None,
            tried: true,
        });
        flags.lock().unwrap().push(w.stop_flag());
        w
    };
    Remotes::start(Box::new(OneMachine), Box::new(spawn))
}

#[test]
fn a_refresh_merges_the_machines_and_marks_what_changed() {
    let dir = tempfile::tempdir().unwrap();
    let status = Arc::new(Mutex::new("run"));
    let s = status.clone();
    let source = move || Swarm {
        orchestrators: vec![orch("w1:p1", *s.lock().unwrap())],
        ..Swarm::default()
    };
    let starts = Arc::new(AtomicUsize::new(0));
    let (st, flags) = (starts.clone(), Arc::new(Mutex::new(Vec::new())));
    let f = flags.clone();
    let start: StartRemotes = Box::new(move || {
        st.fetch_add(1, Ordering::SeqCst);
        remotes(f)
    });
    let mut p = Producer::new(
        Box::new(source),
        Some(start),
        tracker(dir.path()),
        Box::new(|| 1.0),
    );
    assert_eq!(starts.load(Ordering::SeqCst), 0); // nothing before the first refresh
    let first = p.refresh();
    let panes: Vec<_> = first
        .swarm
        .orchestrators
        .iter()
        .map(|o| o.pane_id.as_str())
        .collect();
    assert_eq!(panes, ["w1:p1", "m1:w1:p1"]);
    assert!(first.marks.is_empty() && first.error.is_none());
    *status.lock().unwrap() = "fail";
    let second = p.refresh();
    assert_eq!(second.marks.get("w1:p1-a"), Some(&unseen::Event::Fail));
    assert_eq!(starts.load(Ordering::SeqCst), 1); // started once
    assert!(!flags.lock().unwrap()[0].is_set());
    drop(p);
    assert!(flags.lock().unwrap()[0].is_set()); // a dropped producer stops its watchers
}

#[test]
fn a_panicking_refresh_keeps_the_last_result_with_its_error() {
    let dir = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let source = move || {
        if c.fetch_add(1, Ordering::SeqCst) == 1 {
            panic!("a broken transcript");
        }
        Swarm {
            orchestrators: vec![orch("w1:p1", "run")],
            ..Swarm::default()
        }
    };
    let mut p = Producer::new(
        Box::new(source),
        None,
        tracker(dir.path()),
        Box::new(|| 1.0),
    );
    let good = p.refresh();
    let bad = p.refresh();
    assert_eq!(bad.error.as_deref(), Some("panic: a broken transcript"));
    assert_eq!(bad.swarm, good.swarm);
    let again = p.refresh();
    assert!(again.error.is_none());
    assert_eq!(again.swarm, good.swarm);
}

/// A machine whose snapshot takes 200 ms; counts the snapshots that finished.
struct Slow(Arc<AtomicUsize>);

impl SnapshotApi for Slow {
    fn machine_snapshot(&self, _: &str) -> Result<Snapshot, String> {
        std::thread::sleep(Duration::from_millis(200));
        self.0.fetch_add(1, Ordering::SeqCst);
        Err("unreachable".into())
    }
}

struct NoSync;

impl Syncer for NoSync {
    fn sync(&self, _: &Machine, _: &[String]) -> Option<String> {
        None
    }
}

struct NoCollect;

impl Collect for NoCollect {
    fn continuations(&mut self, _: &[String]) -> Vec<String> {
        Vec::new()
    }
    fn collect(&mut self, _: &Snapshot) -> Swarm {
        Swarm::default()
    }
}

/// A producer with one slow real watcher; its started-snapshot counter.
fn slow_producer(dir: &std::path::Path) -> (Producer, Arc<AtomicUsize>, Arc<Mutex<Vec<StopFlag>>>) {
    let running: Arc<Mutex<Vec<StopFlag>>> = Arc::default();
    let finished = Arc::new(AtomicUsize::new(0));
    let (r, fin) = (running.clone(), finished.clone());
    let start: StartRemotes = Box::new(move || {
        let spawn = move |m: Machine| {
            let w = Watcher::spawn(Refresher {
                machine: m,
                api: Arc::new(Slow(fin.clone())),
                syncer: Arc::new(NoSync),
                collector: Box::new(NoCollect),
            });
            r.lock().unwrap().push(w.stop_flag());
            w
        };
        Remotes::start(Box::new(OneMachine), Box::new(spawn))
    });
    let mut p = Producer::new(
        Box::new(Swarm::default),
        Some(start),
        tracker(dir),
        Box::new(|| 1.0),
    );
    p.refresh();
    std::thread::sleep(Duration::from_millis(50)); // the watcher's first snapshot is under way
    (p, finished, running)
}

#[test]
fn stop_waits_for_the_watcher_threads() {
    let dir = tempfile::tempdir().unwrap();
    let (mut p, finished, running) = slow_producer(dir.path());
    let started = Instant::now();
    p.stop();
    assert!(running.lock().unwrap()[0].is_set());
    // stop returned only after that snapshot finished: the thread was joined
    assert_eq!(finished.load(Ordering::SeqCst), 1);
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(p.remotes.is_none());
}

#[test]
fn a_dropped_producer_waits_for_its_watcher_threads_too() {
    let dir = tempfile::tempdir().unwrap();
    let (p, finished, running) = slow_producer(dir.path());
    drop(p);
    assert!(running.lock().unwrap()[0].is_set());
    assert_eq!(finished.load(Ordering::SeqCst), 1);
}
