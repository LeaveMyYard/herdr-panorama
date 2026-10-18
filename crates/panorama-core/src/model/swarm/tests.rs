//! The model against what the Python code captured (tests/fixtures/README.md): the wire form
//! of the model-built swarms (`inputs/swarm-*.json`, `protocol.swarm_to_wire`) and the model
//! goldens (`model/*.json`, `panorama snapshot --json`, whose orchestrators are the same
//! dataclasses as on the wire, plus Python's `counts`).

use super::*;
use crate::model::Status;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn fixtures(dir: &str) -> Vec<(String, Value)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(dir);
    let mut out: Vec<(String, Value)> = fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("{}: {e}", root.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            (
                name,
                serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap(),
            )
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(!out.is_empty(), "no fixtures in {}", root.display());
    out
}

/// Every captured swarm on the wire, by name.
fn wire_swarms() -> Vec<(String, Value)> {
    fixtures("inputs")
        .into_iter()
        .filter(|(name, _)| name.starts_with("swarm-"))
        .map(|(name, v)| (name, v["swarm"].clone()))
        .collect()
}

/// Every snapshot of every model golden, by name, with its `counts` taken out.
fn snapshots() -> Vec<(String, Value, Value)> {
    let mut out = vec![];
    for (name, golden) in fixtures("model") {
        for (at, snap) in golden["snapshots"].as_object().unwrap() {
            let mut snap = snap.clone();
            let counts = snap.as_object_mut().unwrap().remove("counts").unwrap();
            out.push((format!("{name} {at}"), snap, counts));
        }
    }
    assert!(!out.is_empty());
    out
}

#[test]
fn every_captured_swarm_reads_and_writes_back_unchanged() {
    // nothing dropped, renamed or retyped: the model holds every field Python sends
    for (name, wire) in wire_swarms() {
        let swarm: Swarm =
            serde_json::from_value(wire.clone()).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!swarm.orchestrators.is_empty(), "{name}");
        assert_eq!(serde_json::to_value(&swarm).unwrap(), wire, "{name}");
    }
    for (name, snap, _) in snapshots() {
        let swarm: Swarm =
            serde_json::from_value(snap.clone()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut back = serde_json::to_value(&swarm).unwrap();
        // a snapshot carries no stale_after (`Swarm.to_json` leaves it out): the default is written
        assert_eq!(
            back.as_object_mut().unwrap().remove("stale_after"),
            Some(STALE_AFTER.into())
        );
        assert_eq!(back, snap, "{name}");
    }
}

#[test]
fn counts_are_pythons() {
    for (name, snap, want) in snapshots() {
        let swarm: Swarm = serde_json::from_value(snap).unwrap();
        let c = swarm.counts();
        let got = serde_json::json!({
            "orchestrators": c.orchestrators, "threads": c.threads,
            "active_threads": c.active_threads, "agents": c.agents, "running": c.running,
            "waiting": c.waiting, "questions": c.questions, "blocked": c.blocked,
            "failed": c.failed,
        });
        assert_eq!(got, want, "{name}");
    }
}

#[test]
fn missing_fields_take_model_pys_defaults_and_unknown_ones_are_ignored() {
    let swarm: Swarm = serde_json::from_str(
        r#"{"orchestrators": [{"pane_id": "w1:p1", "tab_id": null, "tab_label": "x",
            "session_id": null, "a_newer_field": 1,
            "threads": [{"id": "t", "name": "n", "source": "turn",
                         "agents": [{"id": "a"}], "phases": [{"name": "p", "state": "now"}]}]}],
            "from_a_newer_minor": {"x": 1}}"#,
    )
    .unwrap();
    assert!(swarm.herdr_ok && !swarm.inbox_ok);
    assert_eq!(swarm.stale_after, STALE_AFTER);
    let o = &swarm.orchestrators[0];
    assert_eq!(o.herdr_status, HerdrStatus::Unknown);
    assert_eq!(o.flavour, "agents");
    let t = &o.threads[0];
    assert_eq!(t.status, ThreadStatus::Active);
    assert!(!t.is_done());
    let a = &t.agents[0];
    assert_eq!(
        (a.role.as_str(), &a.status, a.depth),
        ("agent", &Status::Running, 1)
    );
    assert_eq!(
        a,
        &Agent {
            id: "a".into(),
            ..Agent::default()
        }
    );
    assert_eq!(o.agents().count(), 1);
}

#[test]
fn a_zero_or_null_stale_after_reads_as_the_default() {
    // Python: float(d.get("stale_after") or sw.stale_after)
    for v in ["0", "0.0", "null"] {
        let s: Swarm = serde_json::from_str(&format!(r#"{{"stale_after": {v}}}"#)).unwrap();
        assert_eq!(s.stale_after, STALE_AFTER, "{v}");
    }
    let s: Swarm = serde_json::from_str(r#"{"stale_after": 60}"#).unwrap();
    assert_eq!(s.stale_after, 60.0);
}

#[test]
fn an_agent_without_an_id_or_a_thread_without_its_source_does_not_read() {
    // model.py makes these required: Python's swarm_from_wire raises on them
    assert!(serde_json::from_str::<Agent>(r#"{"role": "x"}"#).is_err());
    assert!(serde_json::from_str::<Thread>(r#"{"id": "t", "name": "n"}"#).is_err());
}

#[test]
fn done_means_done_or_cancelled() {
    for (status, done) in [
        ("active", false),
        ("done", true),
        ("failed", false),
        ("cancelled", true),
    ] {
        let t: Thread = serde_json::from_value(serde_json::json!(
            {"id": "t", "name": "n", "source": "turn", "status": status}))
        .unwrap();
        assert_eq!(t.is_done(), done, "{status}");
    }
}
