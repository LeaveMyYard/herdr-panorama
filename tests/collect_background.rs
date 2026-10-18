//! Background work in the model (Python `test_background.py`, its collector rows) over the
//! captured background scenario: the pane's Claude process is asked of herdr only when a
//! task is open and the process tree can be read, never for another machine's transcripts.

mod collect_support;

use collect_support::{STALE, Scenario};
use panorama::collect::{Collector, Options};
use panorama_core::model::{Orchestrator, Status, Swarm};
use std::sync::{Arc, Mutex};

const WINDOW: f64 = 3600.0;

fn collector(
    sc: &Scenario,
    procs: bool,
    asked: Arc<Mutex<Vec<String>>>,
    pid: Option<u32>,
) -> Collector {
    Collector::new(Options {
        projects: sc.projects(),
        events_dir: sc.root().join("events"),
        hooks_dir: sc.root().join("hooks"),
        procs,
        pane_pid: Box::new(move |pane| {
            asked.lock().unwrap().push(pane.to_owned());
            pid
        }),
    })
}

fn release(sw: &Swarm) -> &Orchestrator {
    sw.orchestrators
        .iter()
        .find(|o| o.pane_id == "w1:p1")
        .unwrap()
}

fn bg_ids(o: &Orchestrator) -> Vec<&str> {
    o.bg.iter().map(|b| b.id.as_str()).collect()
}

#[test]
fn collect_fills_bg_last_activity_and_stale_after() {
    let sc = Scenario::new("background");
    let asked = Arc::new(Mutex::new(Vec::new()));
    let mut c = collector(&sc, true, asked.clone(), Some(std::process::id()));
    let sw = c.collect(Some(sc.snapshot()), None, sc.now(), WINDOW, 7200.0);
    assert_eq!(sw.stale_after, 7200.0);
    assert!(release(&sw).last_activity.is_some());
    assert_eq!(*asked.lock().unwrap(), ["w1:p1"]); // only the pane with open tasks, once
    let ids = bg_ids(release(&sw));
    if cfg!(target_os = "linux") {
        // our own process holds no task's output: its Bash tasks are over; a Monitor is not
        // visible this way (its timeout decides)
        assert_eq!(ids, ["mon1errors"]);
    }
    c.collect(Some(sc.snapshot()), None, sc.now() + 1.0, WINDOW, 7200.0);
    assert_eq!(asked.lock().unwrap().len(), 1); // the process is remembered while it is the same

    let never = Arc::new(Mutex::new(Vec::new()));
    let mut remote = collector(&sc, false, never.clone(), None);
    let sw = remote.collect(Some(sc.snapshot()), None, sc.now(), WINDOW, STALE);
    let o = release(&sw);
    let got: Vec<_> =
        o.bg.iter()
            .map(|b| (b.id.as_str(), b.kind.as_str()))
            .collect();
    assert_eq!(
        got,
        [
            ("bq1rollout", "Bash"),
            ("bq2smoke", "Bash"),
            ("mon1errors", "Monitor")
        ]
    );
    assert!(never.lock().unwrap().is_empty());
}

#[test]
fn a_pane_herdr_cannot_name_is_not_asked_again_at_once() {
    let sc = Scenario::new("background");
    let asked = Arc::new(Mutex::new(Vec::new()));
    let mut c = collector(&sc, true, asked.clone(), None);
    let first = c.collect(Some(sc.snapshot()), None, sc.now(), WINDOW, STALE);
    c.collect(Some(sc.snapshot()), None, sc.now() + 10.0, WINDOW, STALE);
    assert_eq!(asked.lock().unwrap().len(), 1);
    c.collect(Some(sc.snapshot()), None, sc.now() + 31.0, WINDOW, STALE);
    assert_eq!(asked.lock().unwrap().len(), 2);
    // unknown: the transcripts decide, as the golden (herdr named no process) shows
    assert_eq!(
        bg_ids(release(&first)),
        ["bq1rollout", "bq2smoke", "mon1errors"]
    );
}

#[test]
fn a_waiting_agent_says_so_in_the_model() {
    let sc = Scenario::new("background");
    let mut c = collector(&sc, false, Arc::default(), None);
    let sw = c.collect(Some(sc.snapshot()), None, sc.now(), WINDOW, STALE);
    let ag = release(&sw)
        .agents()
        .find(|a| a.id.starts_with("g1canary"))
        .unwrap();
    assert_eq!(ag.status, Status::Background);
    assert_eq!(
        ag.bg.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(),
        ["gb1canary"]
    );
    assert_eq!(ag.activity, "turn ended · 1 task in background");
    assert_eq!(sw.counts().waiting, 2);
}
