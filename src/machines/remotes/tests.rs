use super::*;
use crate::machines::Outcome;
use crate::machines::watcher::StopFlag;
use herdr_agent_kit::herdr_api::Machine as Saved;
use panorama_core::model::Orchestrator;
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;

type Shared = Arc<Mutex<(Selection, Vec<Saved>)>>;

/// The setting and herdr's machines, as a test sets them; counts the herdr calls.
struct Scripted(Shared, Arc<Mutex<usize>>);

impl Source for Scripted {
    fn selection(&mut self) -> Selection {
        self.0.lock().unwrap().0.clone()
    }
    fn machines(&mut self) -> Vec<Saved> {
        *self.1.lock().unwrap() += 1;
        self.0.lock().unwrap().1.clone()
    }
}

fn row(id: &str, label: &str, target: &str) -> Saved {
    serde_json::from_value(json!({"id": id, "label": label, "target": target, "enabled": true}))
        .unwrap()
}

type Started = Arc<Mutex<Vec<(String, StopFlag)>>>;

fn remotes(sel: Selection, rows: Vec<Saved>) -> (Remotes, Shared, Started) {
    let shared: Shared = Arc::new(Mutex::new((sel, rows)));
    let started: Started = Arc::default();
    let log = started.clone();
    let spawn = move |m: Machine| {
        let w = Watcher::idle(m);
        log.lock()
            .unwrap()
            .push((w.machine.id.clone(), w.stop_flag()));
        w
    };
    let r = Remotes::start(
        Box::new(Scripted(shared.clone(), Arc::default())),
        Box::new(spawn),
    );
    (r, shared, started)
}

fn orch(pane: &str, label: &str) -> Orchestrator {
    serde_json::from_value(
        json!({"pane_id": pane, "tab_id": null, "tab_label": label, "session_id": null}),
    )
    .unwrap()
}

fn swarm(panes: &[&str]) -> Swarm {
    Swarm {
        orchestrators: panes.iter().map(|p| orch(p, "x")).collect(),
        ..Swarm::default()
    }
}

#[test]
fn merge_marks_stale_and_unreachable() {
    let rows = vec![row("abc123", "devbox", "devbox"), row("x", "lab", "lab")];
    let (mut r, _, _) = remotes(Selection::All, rows);
    let now = Instant::now();
    r.watchers[0].set_outcome(Outcome {
        swarm: Some(swarm(&["devbox:w1:p1"])),
        at: Some(now),
        error: None,
        tried: true,
    });
    r.watchers[1].set_outcome(Outcome {
        error: Some("ssh: connect timed out".into()),
        tried: true,
        ..Outcome::default()
    });
    let mut local = swarm(&["w1:p1"]);
    r.merge_at(&mut local, now);
    let panes: Vec<_> = local
        .orchestrators
        .iter()
        .map(|o| o.pane_id.as_str())
        .collect();
    assert_eq!(panes, ["w1:p1", "devbox:w1:p1"]);
    assert_eq!(local.errors, ["lab: unreachable (ssh: connect timed out)"]);
    let mut later = Swarm::default(); // no answer for a minute
    r.merge_at(&mut later, now + Duration::from_secs(60));
    assert!(
        later
            .errors
            .contains(&"devbox: stale (no answer)".to_owned())
    );
}

#[test]
fn a_failed_refresh_with_a_last_result_is_stale_with_its_error() {
    let (mut r, _, _) = remotes(Selection::All, vec![row("a", "devbox", "devbox")]);
    let now = Instant::now();
    let mut s = swarm(&["a:w1:p1"]);
    s.errors.push("devbox: transcripts not synced: x".into());
    r.watchers[0].set_outcome(Outcome {
        swarm: Some(s),
        at: Some(now),
        error: Some("unreachable".into()),
        tried: true,
    });
    let mut local = Swarm::default();
    r.merge_at(&mut local, now);
    assert_eq!(
        local.errors,
        [
            "devbox: transcripts not synced: x",
            "devbox: stale (unreachable)"
        ]
    );
    assert_eq!(local.orchestrators.len(), 1);
}

#[test]
fn a_machine_not_tried_yet_adds_nothing() {
    let (mut r, _, _) = remotes(Selection::All, vec![row("a", "devbox", "devbox")]);
    let mut local = Swarm::default();
    r.merge(&mut local);
    assert!(local.errors.is_empty() && local.orchestrators.is_empty());
}

fn ids(started: &Started) -> Vec<String> {
    started
        .lock()
        .unwrap()
        .iter()
        .map(|(id, _)| id.clone())
        .collect()
}

fn stopped(started: &Started, i: usize) -> bool {
    started.lock().unwrap()[i].1.is_set()
}

#[test]
fn a_changed_machines_setting_applies_without_a_restart() {
    let (mut r, shared, started) = remotes(Selection::All, vec![row("abc123", "devbox", "devbox")]);
    assert_eq!(ids(&started), ["abc123"]);
    r.merge(&mut Swarm::default());
    assert_eq!(ids(&started), ["abc123"]); // unchanged setting: same watcher
    assert_eq!(r.watchers.len(), 1);
    assert!(!stopped(&started, 0));
    shared.lock().unwrap().0 = Selection::None;
    r.merge(&mut Swarm::default());
    assert!(r.watchers.is_empty() && stopped(&started, 0));
}

#[test]
fn a_reload_keeps_unchanged_watchers_and_never_runs_two_per_machine() {
    let devbox = row("abc123", "devbox", "devbox");
    let lab = row("def456", "lab", "lab");
    let (mut r, shared, started) = remotes(
        Selection::Labels(vec!["devbox".into()]),
        vec![devbox.clone()],
    );
    *shared.lock().unwrap() = (
        Selection::Labels(vec!["devbox".into(), "lab".into()]),
        vec![devbox.clone(), lab.clone()],
    );
    r.merge(&mut Swarm::default());
    assert!(!stopped(&started, 0)); // devbox kept running, not restarted
    assert_eq!(ids(&started), ["abc123", "def456"]); // only the added one started
    *shared.lock().unwrap() = (Selection::Labels(vec!["lab".into()]), vec![lab]);
    r.merge(&mut Swarm::default());
    let ids: Vec<_> = r.watchers.iter().map(|w| w.machine.id.as_str()).collect();
    assert_eq!(ids, ["def456"]);
    assert!(stopped(&started, 0) && !stopped(&started, 1));
    assert_eq!(started.lock().unwrap().len(), 2);
}

#[test]
fn a_machine_whose_target_changed_gets_a_new_watcher() {
    let (mut r, shared, started) = remotes(Selection::All, vec![row("a", "devbox", "old-host")]);
    *shared.lock().unwrap() = (
        Selection::Labels(vec!["devbox".into()]),
        vec![row("a", "devbox", "new-host")],
    );
    r.merge(&mut Swarm::default());
    assert_eq!(ids(&started), ["a", "a"]);
    assert!(stopped(&started, 0) && !stopped(&started, 1));
    assert_eq!(r.watchers[0].machine.target, "new-host");
}

#[test]
fn an_unchanged_setting_never_asks_herdr_again() {
    let shared: Shared = Arc::new(Mutex::new((
        Selection::All,
        vec![row("a", "devbox", "devbox")],
    )));
    let asked: Arc<Mutex<usize>> = Arc::default();
    let source = Scripted(shared.clone(), asked.clone());
    let mut r = Remotes::start(Box::new(source), Box::new(Watcher::idle));
    assert_eq!(*asked.lock().unwrap(), 1);
    for _ in 0..3 {
        r.merge(&mut Swarm::default());
    }
    assert_eq!(*asked.lock().unwrap(), 1); // the setting is re-read, herdr is not
    shared.lock().unwrap().0 = Selection::None;
    r.merge(&mut Swarm::default());
    assert_eq!(*asked.lock().unwrap(), 1); // "none": no machine list needed
    shared.lock().unwrap().0 = Selection::All;
    r.merge(&mut Swarm::default());
    assert_eq!(*asked.lock().unwrap(), 2);
    assert_eq!(r.watchers.len(), 1);
}

#[test]
fn one_watcher_per_machine_id_the_last_entry_winning() {
    let (mut r, shared, started) = remotes(Selection::None, vec![]);
    *shared.lock().unwrap() = (
        Selection::All,
        vec![
            row("a", "one", "old"),
            row("b", "two", "b"),
            row("a", "one", "new"),
        ],
    );
    r.merge(&mut Swarm::default());
    assert_eq!(ids(&started), ["a", "b"]);
    assert_eq!(r.watchers[0].machine.target, "new");
}
