use super::*;
use panorama_core::model::{Agent, Orchestrator, Status, Swarm, Thread};
use serde_json::json;

fn agent(id: &str, st: &str, qids: &[i64], ended: Option<f64>) -> Agent {
    Agent {
        id: id.into(),
        status: Status::from_code(st),
        question_ids: qids.to_vec(),
        ended,
        ..Agent::default()
    }
}

fn a(id: &str, st: &str) -> Agent {
    agent(id, st, &[], None)
}

fn orch(agents: Vec<Agent>, pane: &str, focused: bool) -> Orchestrator {
    let thread: Thread =
        serde_json::from_value(json!({"id": "t1", "name": "t", "source": "turn"})).unwrap();
    let mut o: Orchestrator =
        serde_json::from_value(json!({"pane_id": pane, "tab_id": "t", "tab_label": "o", "session_id": "s", "focused": focused})).unwrap();
    o.threads = vec![Thread { agents, ..thread }];
    o
}

fn swarm(agents: Vec<Agent>) -> Swarm {
    Swarm {
        orchestrators: vec![orch(agents, "w1:p1", false)],
        ..Swarm::default()
    }
}

fn focused(agents: Vec<Agent>) -> Swarm {
    Swarm {
        orchestrators: vec![orch(agents, "w1:p1", true)],
        ..Swarm::default()
    }
}

fn marks(v: &[(&str, Event)]) -> Marks {
    v.iter().map(|(k, e)| ((*k).to_owned(), *e)).collect()
}

fn tracker(dir: &std::path::Path) -> Tracker {
    Tracker::new(dir.join("unseen.json"), Box::new(|| true))
}

use Event::{Done, Fail, Wait};

#[test]
fn the_first_look_marks_nothing_then_changes_are_events() {
    let dir = tempfile::tempdir().unwrap();
    let t = tracker(dir.path());
    assert!(
        t.update(
            &swarm(vec![a("a", "run"), a("b", "done"), a("c", "fail")]),
            100.0
        )
        .is_empty()
    );
    let m = t.update(
        &swarm(vec![
            a("a", "fail"),
            a("b", "done"),
            a("c", "fail"),
            a("d", "run"),
        ]),
        101.0,
    );
    assert_eq!(m, marks(&[("a", Fail)]));
    let m = t.update(
        &swarm(vec![a("a", "fail"), agent("d", "wait", &[7], None)]),
        102.0,
    );
    assert_eq!(m, marks(&[("a", Fail), ("d", Wait)]));
    let m = t.update(
        &swarm(vec![a("a", "fail"), agent("d", "wait", &[7, 8], None)]),
        103.0,
    );
    assert_eq!(m, marks(&[("a", Fail), ("d", Wait)]));
    let m = t.update(&swarm(vec![a("a", "fail"), a("d", "done")]), 103.5);
    assert_eq!(m, marks(&[("a", Fail), ("d", Done)])); // answered (seen there), then finished
    t.seen(["d"]);
    assert_eq!(
        t.update(&swarm(vec![a("a", "fail"), a("d", "done")]), 104.0),
        marks(&[("a", Fail)])
    );
}

#[test]
fn most_urgent_wins_and_an_answered_question_clears() {
    let dir = tempfile::tempdir().unwrap();
    let t = tracker(dir.path());
    t.update(&swarm(vec![a("a", "run")]), 1.0);
    assert_eq!(
        t.update(&swarm(vec![agent("a", "wait", &[3], None)]), 2.0),
        marks(&[("a", Wait)])
    );
    assert!(t.update(&swarm(vec![a("a", "run")]), 3.0).is_empty()); // answered where it was asked
    assert_eq!(
        t.update(&swarm(vec![a("a", "done")]), 4.0),
        marks(&[("a", Done)])
    );
    assert_eq!(
        t.update(&swarm(vec![a("a", "fail")]), 5.0),
        marks(&[("a", Fail)])
    );
    assert_eq!(
        t.update(&swarm(vec![a("a", "fail"), a("b", "run")]), 6.0),
        marks(&[("a", Fail)])
    );
}

#[test]
fn marks_survive_a_restart_and_are_shared_by_every_panorama() {
    let dir = tempfile::tempdir().unwrap();
    let one = tracker(dir.path());
    one.update(&swarm(vec![a("a", "run"), a("b", "run")]), 1.0);
    one.update(&swarm(vec![a("a", "fail"), a("b", "done")]), 2.0);
    let two = tracker(dir.path()); // a restart, or the home tab next to a popup
    assert_eq!(
        two.update(&swarm(vec![a("a", "fail"), a("b", "done")]), 3.0),
        marks(&[("a", Fail), ("b", Done)])
    );
    two.save_seen(&two.seen(["a"]));
    assert_eq!(
        one.update(&swarm(vec![a("a", "fail"), a("b", "done")]), 4.0),
        marks(&[("b", Done)])
    );
}

#[test]
fn what_happened_while_panorama_was_closed_counts() {
    let dir = tempfile::tempdir().unwrap();
    tracker(dir.path()).update(&swarm(vec![a("a", "run")]), 100.0);
    let later = tracker(dir.path());
    let m = later.update(
        &swarm(vec![
            a("a", "run"),
            agent("old", "done", &[], Some(50.0)),
            agent("new", "done", &[], Some(150.0)),
        ]),
        200.0,
    );
    assert_eq!(m, marks(&[("new", Done)]));
}

#[test]
fn focus_moving_onto_the_orchestrator_pane_clears_its_marks() {
    let dir = tempfile::tempdir().unwrap();
    let t = tracker(dir.path());
    t.update(&swarm(vec![a("a", "run")]), 1.0);
    assert_eq!(
        t.update(&swarm(vec![a("a", "fail")]), 2.0),
        marks(&[("a", Fail)])
    );
    assert!(t.update(&focused(vec![a("a", "fail")]), 3.0).is_empty());
}

#[test]
fn review_probe_a_failure_under_resting_focus_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let t = tracker(dir.path());
    t.update(&focused(vec![a("a", "run")]), 1.0);
    assert_eq!(
        t.update(&focused(vec![a("a", "fail")]), 2.0),
        marks(&[("a", Fail)])
    );
    assert_eq!(
        t.update(&focused(vec![a("a", "fail")]), 2.5),
        marks(&[("a", Fail)])
    ); // still resting there
    assert_eq!(
        t.update(&swarm(vec![a("a", "fail")]), 3.0),
        marks(&[("a", Fail)])
    );
    assert!(t.update(&focused(vec![a("a", "fail")]), 4.0).is_empty()); // now focus arrives
}

#[test]
fn review_probe_the_first_refresh_after_start_never_clears_by_focus() {
    let dir = tempfile::tempdir().unwrap();
    let before = tracker(dir.path());
    before.update(&swarm(vec![a("a", "run")]), 1.0);
    before.update(&swarm(vec![a("a", "fail")]), 2.0);
    let restarted = tracker(dir.path()); // panorama starts with that pane focused
    assert_eq!(
        restarted.update(&focused(vec![a("a", "fail")]), 3.0),
        marks(&[("a", Fail)])
    );
    assert_eq!(
        restarted.update(&focused(vec![a("a", "fail"), a("b", "run")]), 4.0),
        marks(&[("a", Fail)])
    );
}

#[test]
fn review_probe_the_cockpit_focus_on_the_docked_agent_is_not_looking() {
    let dir = tempfile::tempdir().unwrap();
    let t = tracker(dir.path());
    t.update(&focused(vec![a("a", "run"), a("b", "run")]), 1.0);
    assert_eq!(
        t.update(&focused(vec![a("a", "done"), a("b", "run")]), 2.0),
        marks(&[("a", Done)])
    );
    let m = t.update(
        &focused(vec![a("a", "done"), agent("b", "wait", &[4], None)]),
        3.0,
    );
    assert_eq!(m, marks(&[("a", Done), ("b", Wait)]));
}

fn file(t: &Tracker) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(t.path()).unwrap()).unwrap()
}

#[test]
fn remote_agents_are_keyed_by_machine_in_the_state_file() {
    let dir = tempfile::tempdir().unwrap();
    let t = tracker(dir.path());
    let mut o = orch(vec![a("a", "run")], "m1:w1:p1", false);
    o.machine = Some("box".into());
    o.machine_id = Some("m1".into());
    let sw = |o: &Orchestrator| Swarm {
        orchestrators: vec![o.clone()],
        ..Swarm::default()
    };
    t.update(&sw(&o), 1.0);
    o.threads[0].agents[0].status = Status::Failed;
    assert_eq!(t.update(&sw(&o), 2.0), marks(&[("a", Fail)]));
    let keys: Vec<String> = file(&t)["agents"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys, ["m1:a"]);
    t.save_seen(&t.seen(["a"]));
    assert_eq!(file(&t)["agents"]["m1:a"]["ev"], serde_json::Value::Null);
}

#[test]
fn turned_off_marks_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let t = Tracker::new(dir.path().join("u.json"), Box::new(|| false));
    t.update(&swarm(vec![a("a", "run")]), 1.0);
    assert!(t.update(&swarm(vec![a("a", "fail")]), 2.0).is_empty());
    assert!(!enabled(
        json!({"highlight_unseen": false}).as_object().unwrap()
    ));
    assert!(enabled(json!({}).as_object().unwrap())); // the default
    assert!(enabled(
        json!({"highlight_unseen": "no"}).as_object().unwrap()
    )); // not a bool: the default
}

#[test]
fn a_busy_lock_keeps_the_seen_mark_until_it_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let t = tracker(dir.path());
    t.update(&swarm(vec![a("a", "run")]), 1.0);
    t.update(&swarm(vec![a("a", "fail")]), 2.0);
    // another panorama holds the lock
    let lock = herdr_agent_kit::plugin_env::LockFile::new(dir.path().join("unseen.json.lock"))
        .exclusive()
        .unwrap();
    t.save_seen(&t.seen(["a"]));
    assert!(t.marks().is_empty());
    assert!(t.update(&swarm(vec![a("a", "fail")]), 2.5).is_empty()); // the lock is still busy: last marks
    drop(lock);
    assert!(t.update(&swarm(vec![a("a", "fail")]), 3.0).is_empty()); // pending: cleared now
    assert_eq!(file(&t)["agents"]["a"]["ev"], serde_json::Value::Null);
}

#[test]
fn a_broken_state_file_starts_over() {
    let dir = tempfile::tempdir().unwrap();
    let t = tracker(dir.path());
    std::fs::write(t.path(), "{not json").unwrap();
    assert!(t.update(&swarm(vec![a("a", "fail")]), 1.0).is_empty());
}

#[test]
fn old_agents_are_forgotten() {
    let dir = tempfile::tempdir().unwrap();
    let t = tracker(dir.path());
    t.update(&swarm(vec![a("a", "run")]), 0.0);
    t.update(&swarm(vec![a("b", "run")]), KEEP_SECONDS + 1.0);
    let keys: Vec<String> = file(&t)["agents"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys, ["b"]);
}

#[test]
fn focus_on_another_machine_s_pane_is_not_looking() {
    let dir = tempfile::tempdir().unwrap();
    let t = tracker(dir.path());
    let remote = |st: &str, focused: bool| {
        let mut o = orch(vec![a("a", st)], "m1:w1:p1", focused);
        o.machine = Some("box".into());
        o.machine_id = Some("m1".into());
        Swarm {
            orchestrators: vec![o],
            ..Swarm::default()
        }
    };
    t.update(&remote("run", false), 1.0);
    assert_eq!(t.update(&remote("fail", false), 2.0), marks(&[("a", Fail)]));
    // that machine's herdr focus is not this herdr's: the mark stays
    assert_eq!(t.update(&remote("fail", true), 3.0), marks(&[("a", Fail)]));
}
