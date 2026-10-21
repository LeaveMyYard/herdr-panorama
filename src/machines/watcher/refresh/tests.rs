use super::*;
use panorama_core::model::Orchestrator;
use serde_json::json;
use std::sync::Mutex;

pub(crate) const OLD: &str = "77777777-7777-4777-8777-777777777777";
pub(crate) const NEW: &str = "88888888-8888-4888-8888-888888888888";

pub(crate) fn snap(v: serde_json::Value) -> Snapshot {
    serde_json::from_value(v).unwrap()
}

/// `test_machines.py`'s snapshot of one agent in w1:t1.
pub(crate) fn one_agent(session: Option<&str>) -> Snapshot {
    let mut agent =
        json!({"pane_id": "w1:p1", "tab_id": "w1:t1", "workspace_id": "w1", "agent": "claude"});
    if let Some(s) = session {
        agent["agent_session"] = json!({"kind": "id", "value": s});
    }
    snap(
        json!({"tabs": [{"tab_id": "w1:t1", "label": "work", "workspace_id": "w1"}], "agents": [agent]}),
    )
}

pub(crate) struct Api(pub Mutex<Result<Snapshot, String>>);

impl SnapshotApi for Api {
    fn machine_snapshot(&self, _: &str) -> Result<Snapshot, String> {
        self.0.lock().unwrap().clone()
    }
}

#[derive(Default)]
pub(crate) struct Syncs {
    pub calls: Mutex<Vec<Vec<String>>>,
    pub error: Mutex<Option<String>>,
}

impl Syncer for Syncs {
    fn sync(&self, _: &Machine, sessions: &[String], _: &dyn Fn() -> bool) -> Option<String> {
        let mut sorted = sessions.to_vec();
        sorted.sort();
        self.calls.lock().unwrap().push(sorted);
        self.error.lock().unwrap().clone()
    }
}

/// A collector whose transcripts say `OLD` continued in `NEW` once `OLD` was mirrored; its
/// swarm has one orchestrator per agent of the snapshot, showing the latest session.
pub(crate) struct Mirrorlike {
    pub syncs: Arc<Syncs>,
}

impl Collect for Mirrorlike {
    fn continuations(&mut self, named: &[String]) -> Vec<String> {
        let mirrored = self
            .syncs
            .calls
            .lock()
            .unwrap()
            .iter()
            .flatten()
            .any(|s| s == OLD);
        if mirrored && named.iter().any(|s| s == OLD) {
            vec![NEW.into()]
        } else {
            vec![]
        }
    }
    fn collect(&mut self, snapshot: &Snapshot) -> Swarm {
        let has_new = self
            .syncs
            .calls
            .lock()
            .unwrap()
            .iter()
            .flatten()
            .any(|s| s == NEW);
        let orchestrators = snapshot
            .agents
            .iter()
            .map(|a| {
                let sid = if has_new { Some(NEW) } else { a.agent_session.as_ref().and_then(|s| s.id()) };
                let o = json!({"pane_id": a.pane_id, "tab_id": a.tab_id, "tab_label": "work", "session_id": sid});
                serde_json::from_value::<Orchestrator>(o).unwrap()
            })
            .collect();
        Swarm {
            orchestrators,
            ..Swarm::default()
        }
    }
}

pub(crate) fn refresher(api: Arc<Api>, syncs: Arc<Syncs>) -> Refresher {
    Refresher {
        machine: Machine::new("abc123", "devbox", "devbox"),
        api,
        syncer: syncs.clone(),
        collector: Box::new(Mirrorlike { syncs }),
    }
}

#[test]
fn a_failed_refresh_keeps_the_last_result() {
    let api = Arc::new(Api(Mutex::new(Ok(one_agent(None)))));
    let mut r = refresher(api.clone(), Arc::default());
    let first = r.refresh(&Outcome::default(), &|| false);
    let pane = |o: &Outcome| o.swarm.as_ref().unwrap().orchestrators[0].pane_id.clone();
    assert_eq!(pane(&first), "abc123:w1:p1");
    assert!(first.tried && first.error.is_none() && first.at.is_some());
    *api.0.lock().unwrap() = Err("unreachable".into());
    let second = r.refresh(&first, &|| false);
    assert_eq!(pane(&second), "abc123:w1:p1");
    assert_eq!(second.error.as_deref(), Some("unreachable"));
    assert_eq!(second.at, first.at);
    *api.0.lock().unwrap() = Err(String::new());
    assert_eq!(
        r.refresh(&second, &|| false).error.as_deref(),
        Some("unreachable")
    );
}

#[test]
fn continuations_are_mirrored_in_the_same_refresh() {
    let api = Arc::new(Api(Mutex::new(Ok(one_agent(Some(OLD))))));
    let syncs: Arc<Syncs> = Arc::default();
    let mut r = refresher(api, syncs.clone());
    let out = r.refresh(&Outcome::default(), &|| false);
    let o = &out.swarm.unwrap().orchestrators[0];
    assert_eq!(o.session_id.as_deref(), Some(NEW));
    let mut both = vec![OLD.to_owned(), NEW.to_owned()];
    both.sort();
    assert_eq!(
        *syncs.calls.lock().unwrap(),
        [vec![OLD.to_owned()], both.clone()]
    );
    r.refresh(&Outcome::default(), &|| false); // known now: one sync with both
    let calls = syncs.calls.lock().unwrap();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[2], both);
}

#[test]
fn a_failed_sync_is_said_and_stops_the_rounds() {
    let api = Arc::new(Api(Mutex::new(Ok(one_agent(Some(OLD))))));
    let syncs: Arc<Syncs> = Arc::default();
    *syncs.error.lock().unwrap() = Some("rsync timed out".into());
    let out = refresher(api, syncs.clone()).refresh(&Outcome::default(), &|| false);
    assert_eq!(syncs.calls.lock().unwrap().len(), 1);
    let swarm = out.swarm.unwrap();
    assert_eq!(
        swarm.errors,
        ["devbox: transcripts not synced: rsync timed out"]
    );
    assert!(out.error.is_none()); // the snapshot came: a good result
}

#[test]
fn only_session_ids_are_named() {
    let s = snap(json!({"agents": [
        {"pane_id": "w1:p1", "agent_session": {"kind": "id", "value": "a"}},
        {"pane_id": "w1:p2", "agent_session": {"value": "b"}},
        {"pane_id": "w1:p3", "agent_session": {"kind": "name", "value": "c"}},
        {"pane_id": "w1:p4", "agent_session": {"kind": "id", "value": ""}},
        {"pane_id": "w1:p5"},
    ]}));
    assert_eq!(session_ids(&s), ["a", "b"]);
}

#[test]
fn herdrs_words_are_kept_for_the_view() {
    assert_eq!(
        error_text(&HerdrError::Garbled("no snapshot".into())),
        "no snapshot"
    );
    let refused = HerdrError::Refused {
        code: String::new(),
        message: "error: unknown machine 'x'".into(),
    };
    assert_eq!(error_text(&refused), "error: unknown machine 'x'");
}

#[test]
fn the_snapshot_is_forwarded_read_only() {
    use herdr_agent_kit::herdr_api::fake::{CliReply, CliStub, FakeHerdr};
    let fake = FakeHerdr::start();
    let mut stub = CliStub::new();
    let reply = r#"{"result": {"snapshot": {"agents": [], "panes": []}}}"#;
    stub.on(
        &["--machine", "abc123", "api", "snapshot"],
        CliReply::ok(reply),
    );
    let h = fake.blocking_client().with_bin(stub.bin());
    let got = SnapshotApi::machine_snapshot(&h, "abc123").unwrap();
    assert!(got.agents.is_empty() && got.panes.is_empty());
    assert_eq!(stub.calls(), [["--machine", "abc123", "api", "snapshot"]]);
    assert!(fake.calls().is_empty()); // nothing on the local socket
}

#[test]
fn an_unknown_machine_says_what_herdr_said() {
    use herdr_agent_kit::herdr_api::fake::{CliStub, FakeHerdr};
    let fake = FakeHerdr::start();
    let stub = CliStub::recorded(); // captured: `--machine nosuch api snapshot`, exit 2
    let h = fake.blocking_client().with_bin(stub.bin());
    let err = SnapshotApi::machine_snapshot(&h, "nosuch").unwrap_err();
    assert_eq!(
        err,
        "error: unknown machine 'nosuch'; use `herdr machine list`"
    );
}
