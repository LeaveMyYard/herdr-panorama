use super::refresh::tests::{Api, Syncs, one_agent, refresher};
use super::*;
use std::time::Instant;

fn wait_for(mut done: impl FnMut() -> bool) -> bool {
    let until = Instant::now() + Duration::from_secs(5);
    while Instant::now() < until {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn a_watcher_refreshes_on_its_own_thread_until_stopped() {
    let api = Arc::new(Api(Mutex::new(Ok(one_agent(None)))));
    let w = Watcher::spawn(refresher(api, Arc::default()));
    assert!(wait_for(|| w.outcome().tried));
    let swarm = w.outcome().swarm.unwrap();
    assert_eq!(swarm.orchestrators[0].pane_id, "abc123:w1:p1");
    assert!(w.is_running());
    let started = Instant::now();
    w.stop();
    assert!(wait_for(|| !w.is_running()));
    assert!(started.elapsed() < REMOTE_SECONDS); // the wait between refreshes ends at once
    assert!(w.is_stopped());
    w.join();
}

struct Panics;

impl SnapshotApi for Panics {
    fn machine_snapshot(&self, _: &str) -> Result<herdr_agent_kit::herdr_api::Snapshot, String> {
        panic!("boom")
    }
}

#[test]
fn a_panicking_refresh_is_that_machines_error() {
    // a panic while another test holds a panic hook would reach that hook
    let _hook = crate::ui::guard::hook_lock();
    let mut r = refresher(
        Arc::new(Api(Mutex::new(Err("x".into())))),
        Arc::<Syncs>::default(),
    );
    r.api = Arc::new(Panics);
    let w = Watcher::spawn(r);
    assert!(wait_for(|| w.outcome().tried));
    assert_eq!(w.outcome().error.as_deref(), Some("panic: boom"));
    assert!(w.outcome().swarm.is_none());
    w.join();
}

#[test]
fn an_idle_watcher_has_tried_nothing() {
    let w = Watcher::idle(Machine::new("a", "b", "c"));
    assert_eq!(w.outcome(), Outcome::default());
    assert!(!w.is_running() && !w.is_stopped());
}

#[test]
fn a_dropped_watcher_stops() {
    let api = Arc::new(Api(Mutex::new(Ok(one_agent(None)))));
    let w = Watcher::spawn(refresher(api, Arc::default()));
    let flag = w.stop_flag();
    assert!(wait_for(|| w.outcome().tried));
    drop(w);
    assert!(flag.is_set());
}
