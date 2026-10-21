//! The refresh thread: one refresh at a time, a cleaned swarm with its marks, a failure as a
//! failed refresh, and quitting that stops the refresher without waiting it out.

use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// A refresher from a closure; `stopped` says whether it was stopped, `halted` whether its
/// halt handle was used.
struct Fake {
    refresh: Box<dyn FnMut() -> Refreshed + Send>,
    stopped: Arc<AtomicBool>,
    halted: Arc<AtomicBool>,
    tracker: Option<Arc<Tracker>>,
}

impl Refresher for Fake {
    fn refresh(&mut self) -> Refreshed {
        (self.refresh)()
    }

    fn stop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
    }

    fn handles(&self) -> Handles {
        let halted = self.halted.clone();
        Handles {
            halt: Box::new(move || halted.store(true, Ordering::SeqCst)),
            tracker: self.tracker.clone(),
        }
    }
}

fn fake(refresh: impl FnMut() -> Refreshed + Send + 'static) -> (Fake, Arc<AtomicBool>) {
    let stopped = Arc::new(AtomicBool::new(false));
    let f = Fake {
        refresh: Box::new(refresh),
        stopped: stopped.clone(),
        halted: Arc::default(),
        tracker: None,
    };
    (f, stopped)
}

fn swarm(sw: Swarm) -> Refreshed {
    Refreshed {
        swarm: sw,
        ..Refreshed::default()
    }
}

fn recv(rx: &mut mpsc::UnboundedReceiver<Collected>) -> Collected {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        tokio::time::timeout(Duration::from_secs(10), rx.recv())
            .await
            .unwrap()
            .unwrap()
    })
}

#[test]
fn one_refresh_at_a_time_its_swarm_cleaned_and_its_marks_kept() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (f, _) = fake(|| {
        let mut out = swarm(Swarm {
            errors: vec!["bad \u{1b}[31mrecord\u{7}".into()],
            docked: Some("w1:p3".into()),
            ..Swarm::default()
        });
        for (id, ev) in [
            ("a1", Event::Fail),
            ("a2", Event::Wait),
            ("a3", Event::Done),
        ] {
            out.marks.insert(id.into(), ev);
        }
        out
    });
    let mut w = Worker::spawn(move || f, tx).unwrap();
    assert!(w.request());
    assert!(!w.request()); // one is running: the tick is skipped
    match recv(&mut rx) {
        Collected::Swarm(sw, marks) => {
            assert!(!sw.errors[0].contains('\u{1b}') && !sw.errors[0].contains('\u{7}'));
            assert_eq!(sw.docked.as_deref(), Some("w1:p3")); // kept through the cleaning
            let want = [
                ("a1", Unseen::Fail),
                ("a2", Unseen::Wait),
                ("a3", Unseen::Done),
            ];
            assert_eq!(marks, want.map(|(i, u)| (i.to_owned(), u)).into());
        }
        other => panic!("{other:?}"),
    }
    w.done();
    assert!(w.request());
    assert!(matches!(recv(&mut rx), Collected::Swarm(..)));
}

#[test]
fn a_failed_or_panicking_refresh_is_a_failed_refresh_and_the_next_one_still_runs() {
    let _hook = crate::ui::guard::hook_lock();
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {})); // keep the test's output quiet
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut n = 0;
    let (f, _) = fake(move || {
        n += 1;
        assert!(n != 1, "bad record");
        Refreshed {
            error: (n == 2).then(|| "panic: a bad transcript".into()),
            ..Refreshed::default()
        }
    });
    let mut w = Worker::spawn(move || f, tx).unwrap();
    let mut next = || {
        w.request();
        let c = recv(&mut rx);
        w.done();
        c
    };
    let (first, second, third) = (next(), next(), next());
    std::panic::set_hook(prev);
    assert!(
        matches!(&first, Collected::Failed(m) if m.contains("bad record")),
        "{first:?}"
    );
    // the producer's own failure (it kept the last good swarm): the header says why
    assert!(
        matches!(&second, Collected::Failed(m) if m == "panic: a bad transcript"),
        "{second:?}"
    );
    assert!(matches!(third, Collected::Swarm(..)));
}

#[test]
fn a_refresher_that_cannot_start_fails_every_refresh() {
    let _hook = crate::ui::guard::hook_lock();
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut w = Worker::spawn(|| -> Fake { panic!("no herdr") }, tx).unwrap();
    let started = recv(&mut rx);
    w.request();
    let asked = recv(&mut rx);
    std::panic::set_hook(prev);
    assert!(
        matches!(&started, Collected::Failed(m) if m.contains("no herdr")),
        "{started:?}"
    );
    assert!(matches!(asked, Collected::Failed(_)));
    assert!(w.stop(Duration::from_secs(10))); // nothing to stop: the thread still ends
}

#[test]
fn quitting_stops_the_refresher() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let (f, stopped) = fake(|| swarm(Swarm::default()));
    let w = Worker::spawn(move || f, tx).unwrap();
    assert!(w.stop(Duration::from_secs(10)));
    assert!(stopped.load(Ordering::SeqCst));
}

#[test]
fn quitting_during_a_long_refresh_does_not_wait_it_out_and_it_stops_after() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let (go, wait) = std::sync::mpsc::channel::<()>();
    let (entered, inside) = std::sync::mpsc::channel::<()>();
    let (f, stopped) = fake(move || {
        entered.send(()).unwrap();
        let _ = wait.recv(); // a refresh stuck in a slow herdr call
        swarm(Swarm::default())
    });
    let halted = f.halted.clone();
    let mut w = Worker::spawn(move || f, tx).unwrap();
    assert!(w.request());
    inside.recv().unwrap();
    let asked = std::time::Instant::now();
    assert!(!w.stop(Duration::from_millis(50)));
    assert!(asked.elapsed() < Duration::from_secs(5));
    // the halt reached the remote watchers from here, the refresh still running
    assert!(halted.load(Ordering::SeqCst));
    assert!(!stopped.load(Ordering::SeqCst));
    go.send(()).unwrap(); // the call returns: the thread stops the refresher on its own
    eventually(&stopped);
}

#[test]
fn an_ended_app_stops_the_refresher() {
    let (tx, rx) = mpsc::unbounded_channel();
    let (f, stopped) = fake(|| swarm(Swarm::default()));
    let mut w = Worker::spawn(move || f, tx).unwrap();
    drop(rx);
    assert!(w.request()); // its outcome has nowhere to go: the thread ends, the worker kept
    eventually(&stopped);
    drop(w);
}

fn eventually(stopped: &AtomicBool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !stopped.load(Ordering::SeqCst) {
        assert!(std::time::Instant::now() < deadline, "never stopped");
        std::thread::yield_now();
    }
}

#[test]
fn the_ui_reaches_the_refreshers_tracker_once_it_is_made() {
    let dir = tempfile::tempdir().unwrap();
    let t = Arc::new(Tracker::new(
        dir.path().join("unseen.json"),
        Box::new(|| true),
    ));
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (mut f, _) = fake(|| swarm(Swarm::default()));
    f.tracker = Some(t.clone());
    let mut w = Worker::spawn(move || f, tx).unwrap();
    w.request();
    recv(&mut rx); // made, and refreshed once
    assert!(Arc::ptr_eq(&w.tracker().unwrap(), &t));
}

#[test]
fn the_state_file_is_read_and_written_off_the_ui_thread() {
    // tests/test_unseen.py::test_review_l2_the_state_file_is_read_and_written_off_the_ui_thread:
    // the real producer's tracker updates on the refresh thread; a seen mark is saved on a
    // thread of its own (`app::seen::mark`), never the caller's
    let dir = tempfile::tempdir().unwrap();
    let threads = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen_on = threads.clone();
    let enabled = Box::new(move || {
        seen_on.lock().unwrap().push(std::thread::current().id());
        true
    });
    let tracker = Arc::new(Tracker::new(dir.path().join("unseen.json"), enabled));
    let producer = crate::produce::Producer::new(
        Box::new(Swarm::default),
        None,
        tracker.clone(),
        Box::new(|| 1.0),
    );
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut w = Worker::spawn(move || producer, tx).unwrap();
    assert!(w.request());
    assert!(matches!(recv(&mut rx), Collected::Swarm(..)));
    let ui = std::thread::current().id();
    let updates = threads.lock().unwrap().clone();
    assert!(
        !updates.is_empty() && updates.iter().all(|t| *t != ui),
        "the tracker updated on the UI thread"
    );
    let marked = Arc::new(Tracker::new(
        dir.path().join("unseen.json"),
        Box::new(|| true),
    ));
    let sw: Swarm = serde_json::from_value(
        serde_json::json!({"orchestrators": [{"pane_id": "w1:p1", "tab_id": "t",
        "tab_label": "o", "session_id": "s", "threads": [{"id": "t1", "name": "t", "source": "turn",
        "agents": [{"id": "a1", "status": "run"}]}]}]}),
    )
    .unwrap();
    marked.update(&sw, 1.0);
    let mut done = sw.clone();
    done.orchestrators[0].threads[0].agents[0].status = panorama_core::model::Status::Done;
    marked.update(&done, 2.0);
    // the save runs off this thread: it waits for the state file's lock this thread holds
    // (a save on this thread gives up while the lock is held, leaving the mark pending)
    let held = herdr_agent_kit::plugin_env::LockFile::new(dir.path().join("unseen.json.lock"))
        .try_exclusive()
        .unwrap()
        .expect("the state file's lock");
    let save = crate::app::seen::mark(&marked, &["a1".into()]).expect("a mark to save");
    drop(held);
    save.join().unwrap();
    let other = Tracker::new(dir.path().join("unseen.json"), Box::new(|| true));
    assert!(
        other.update(&done, 3.0).is_empty(),
        "the save ran on the caller's thread"
    );
    assert!(w.stop(Duration::from_secs(10)));
}
