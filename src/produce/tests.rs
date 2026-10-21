use super::*;
use crate::machines::sync::Syncer;
use crate::machines::{
    Collect, Machine, Outcome, Refresher, Selection, SnapshotApi, Source, StopFlag, Watcher,
};
use herdr_agent_kit::herdr_api::{Machine as Saved, Snapshot};
use herdr_agent_kit::plugin_env::PluginEnv;
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
        remotes(f.clone())
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
    // a panic while another test holds a panic hook would reach that hook
    let _hook = crate::ui::guard::hook_lock();
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
    fn sync(&self, _: &Machine, _: &[String], _: &dyn Fn() -> bool) -> Option<String> {
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
        let (r, fin) = (r.clone(), fin.clone());
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

/// A forwarded snapshot that names one session, so the watcher syncs.
struct Names;

impl SnapshotApi for Names {
    fn machine_snapshot(&self, _: &str) -> Result<Snapshot, String> {
        let v = json!({"agents": [{"pane_id": "w1:p1", "agent": "claude",
            "agent_session": {"kind": "id", "value": "11111111-2222-3333-4444-555555555555"}}]});
        Ok(serde_json::from_value(v).unwrap())
    }
}

/// A producer on `source` whose one machine's sync hangs (the rsync a hung ssh leaves: it
/// never ends on its own, 30 s timeout), refreshed once; the hung child's pid, which only
/// this test started.
fn hung_rsync(
    dir: &std::path::Path,
    source: Box<dyn FnMut() -> Swarm + Send>,
) -> (Producer, rustix::process::Pid) {
    use crate::machines::mirror::run;
    use crate::machines::sync::RsyncSync;
    let pidfile = dir.join("rsync.pid");
    let env = PluginEnv::from_vars([("HERDR_PLUGIN_STATE_DIR", dir.join("state"))], None, 1000);
    let mut syncer = RsyncSync::new(&env, Arc::new(|| 3600.0));
    let pf = pidfile.clone();
    syncer.runner = Arc::new(move |_: &[String], t, cancelled: &dyn Fn() -> bool| {
        let script = format!("sleep 60 & echo $! > {}; wait", pf.display());
        run(&["sh".to_owned(), "-c".to_owned(), script], t, cancelled)
    });
    let syncer = Arc::new(syncer);
    let start: StartRemotes = Box::new(move || {
        let syncer = syncer.clone();
        let spawn = move |m: Machine| {
            Watcher::spawn(Refresher {
                machine: m,
                api: Arc::new(Names),
                syncer: syncer.clone(),
                collector: Box::new(NoCollect),
            })
        };
        Remotes::start(Box::new(OneMachine), Box::new(spawn))
    });
    let mut p = Producer::new(source, Some(start), tracker(dir), Box::new(|| 1.0));
    p.refresh();
    let until = Instant::now() + Duration::from_secs(5);
    while !pidfile.exists() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    let child: i32 = std::fs::read_to_string(&pidfile)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    (p, rustix::process::Pid::from_raw(child).unwrap())
}

/// Whether `pid` is gone by `within` (signal 0 only: nothing is sent to it).
fn gone_within(pid: rustix::process::Pid, within: Duration) -> bool {
    let until = Instant::now() + within;
    while rustix::process::test_kill_process(pid).is_ok() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    rustix::process::test_kill_process(pid).is_err()
}

#[test]
fn stop_kills_a_hung_rsync_instead_of_waiting_it_out() {
    let dir = tempfile::tempdir().unwrap();
    let (p, child) = hung_rsync(dir.path(), Box::new(Swarm::default));
    let started = Instant::now();
    drop(p);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
    assert!(
        gone_within(child, Duration::from_secs(2)),
        "the rsync's group is gone"
    );
}

#[test]
fn quitting_mid_refresh_halts_a_hung_rsync_within_the_stop_wait() {
    // the app's quit while a local refresh hangs (herdr's snapshot timeout): the halt reaches
    // the watcher from the quitting thread, not through the refresh thread's queue
    use crate::app::collect::{Collected, Worker};
    use crate::app::run::STOP_WAIT;
    let dir = tempfile::tempdir().unwrap();
    let (entered, inside) = std::sync::mpsc::channel::<()>();
    let (release, gate) = std::sync::mpsc::channel::<()>();
    let mut n = 0;
    let source = Box::new(move || {
        n += 1;
        if n > 1 {
            let _ = entered.send(());
            let _ = gate.recv_timeout(Duration::from_secs(30)); // a local refresh that hangs
        }
        Swarm::default()
    });
    let (p, child) = hung_rsync(dir.path(), source);
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<Collected>();
    let mut w = Worker::spawn(move || p, tx).unwrap();
    assert!(w.request());
    inside.recv().unwrap(); // the refresh is under way
    let quit = Instant::now();
    assert!(!w.stop(STOP_WAIT)); // not waited out
    assert!(
        gone_within(child, STOP_WAIT.saturating_sub(quit.elapsed())),
        "the hung rsync outlived the quit"
    );
    release.send(()).unwrap();
}

#[test]
fn a_remote_agent_that_fails_is_marked() {
    let dir = tempfile::tempdir().unwrap();
    let flags = Arc::new(Mutex::new(Vec::new()));
    let f = flags.clone();
    let start: StartRemotes = Box::new(move || remotes(f.clone()));
    let mut p = Producer::new(
        Box::new(Swarm::default),
        Some(start),
        tracker(dir.path()),
        Box::new(|| 1.0),
    );
    p.refresh();
    let w = &p.remotes.as_ref().unwrap().watchers[0];
    let mut out = w.outcome();
    out.swarm.as_mut().unwrap().orchestrators[0].threads[0].agents[0].status = Status::Failed;
    w.set_outcome(out);
    let r = p.refresh();
    assert_eq!(r.marks.get("m1:w1:p1-a"), Some(&unseen::Event::Fail)); // marks are of the merged swarm
}

#[test]
fn a_long_panic_is_kept_as_its_first_200_characters() {
    // a panic while another test holds a panic hook would reach that hook
    let _hook = crate::ui::guard::hook_lock();
    let dir = tempfile::tempdir().unwrap();
    let source = || -> Swarm { panic!("{}", "x".repeat(500)) };
    let mut p = Producer::new(
        Box::new(source),
        None,
        tracker(dir.path()),
        Box::new(|| 1.0),
    );
    assert_eq!(p.refresh().error.unwrap().chars().count(), ERROR_CHARS);
}

#[test]
fn a_remote_start_that_panics_is_tried_again_next_refresh() {
    // a panic while another test holds a panic hook would reach that hook
    let _hook = crate::ui::guard::hook_lock();
    let dir = tempfile::tempdir().unwrap();
    let tries = Arc::new(AtomicUsize::new(0));
    let (t, flags) = (tries.clone(), Arc::new(Mutex::new(Vec::new())));
    let start: StartRemotes = Box::new(move || {
        if t.fetch_add(1, Ordering::SeqCst) == 0 {
            panic!("no thread for the watcher");
        }
        remotes(flags.clone())
    });
    let mut p = Producer::new(
        Box::new(Swarm::default),
        Some(start),
        tracker(dir.path()),
        Box::new(|| 1.0),
    );
    assert!(p.refresh().error.is_some());
    let r = p.refresh();
    assert!(r.error.is_none());
    assert_eq!(r.swarm.orchestrators.len(), 1);
    assert_eq!(tries.load(Ordering::SeqCst), 2);
}

#[test]
fn highlight_unseen_follows_the_config() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.toml");
    std::fs::write(&file, "highlight_unseen = false\n").unwrap();
    let enabled = unseen_enabled(ConfigFile::in_dir(dir.path()));
    assert!(!enabled());
    std::fs::write(&file, "").unwrap();
    assert!(enabled()); // the default
}

#[test]
fn after_a_halt_no_watcher_starts_and_one_starting_meanwhile_is_stopped() {
    let dir = tempfile::tempdir().unwrap();
    let flags = Arc::new(Mutex::new(Vec::new()));
    // halted before the first refresh: its remotes never start
    let f = flags.clone();
    let start: StartRemotes = Box::new(move || remotes(f.clone()));
    let mut p = Producer::new(
        Box::new(Swarm::default),
        Some(start),
        tracker(dir.path()),
        Box::new(|| 1.0),
    );
    p.halt().stop();
    p.refresh();
    assert!(p.remotes.is_none() && flags.lock().unwrap().is_empty());
    // halted while the first refresh starts them (the quit races the refresh)
    let late: Arc<Mutex<Option<Halt>>> = Arc::default();
    let (f, l) = (flags.clone(), late.clone());
    let start: StartRemotes = Box::new(move || {
        let r = remotes(f.clone());
        l.lock().unwrap().as_ref().unwrap().stop();
        r
    });
    let mut p = Producer::new(
        Box::new(Swarm::default),
        Some(start),
        tracker(dir.path()),
        Box::new(|| 1.0),
    );
    *late.lock().unwrap() = Some(p.halt());
    p.refresh();
    assert!(flags.lock().unwrap()[0].is_set());
}
