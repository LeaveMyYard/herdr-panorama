//! The model against its fixtures: the Python-captured swarms (`inputs/swarm-*.json`, panel
//! protocol 1.x) read as 2.x through `tests/v1_wire`, and the Rust model goldens
//! (`rust/model/*.json`, `panorama snapshot --json`, whose roots are the model's fields, plus
//! its `counts`).

use super::*;
use crate::model::Status;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[path = "../../../../../tests/v1_wire/mod.rs"]
mod v1_wire;

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

/// Every captured swarm on the wire, by name, in the 2.x form.
fn wire_swarms() -> Vec<(String, Value)> {
    fixtures("inputs")
        .into_iter()
        .filter(|(name, _)| name.starts_with("swarm-"))
        .map(|(name, v)| (name, v1_wire::swarm(v["swarm"].clone())))
        .collect()
}

/// Every snapshot of every Rust model golden, by name, with its `counts` taken out.
fn snapshots() -> Vec<(String, Value, Value)> {
    let mut out = vec![];
    for (name, golden) in fixtures("rust/model") {
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
    // nothing dropped, renamed or retyped: the model holds every field sent
    for (name, mut wire) in wire_swarms() {
        let swarm: Swarm =
            serde_json::from_value(wire.clone()).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!swarm.roots.is_empty(), "{name}");
        for o in wire["roots"].as_array_mut().unwrap() {
            o["harness"] = Value::String("claude".into());
            o["hidden_finished"] = 0.into();
        }
        assert_eq!(serde_json::to_value(&swarm).unwrap(), wire, "{name}");
    }
    for (name, snap, _) in snapshots() {
        let swarm: Swarm =
            serde_json::from_value(snap.clone()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut back = serde_json::to_value(&swarm).unwrap();
        // a snapshot carries no stale_after: the default is written
        assert_eq!(
            back.as_object_mut().unwrap().remove("stale_after"),
            Some(STALE_AFTER.into())
        );
        assert_eq!(back, snap, "{name}");
    }
}

#[test]
fn counts_are_the_goldens() {
    for (name, snap, want) in snapshots() {
        let swarm: Swarm = serde_json::from_value(snap).unwrap();
        assert_eq!(
            serde_json::to_value(swarm.counts()).unwrap(),
            want,
            "{name}"
        );
    }
}

#[test]
fn missing_fields_take_model_pys_defaults_and_unknown_ones_are_ignored() {
    let swarm: Swarm = serde_json::from_str(
        r#"{"roots": [{"pane_id": "w1:p1", "tab_id": null, "tab_label": "x",
            "session_id": null, "a_newer_field": 1, "agents": [{"id": "a"}]}],
            "from_a_newer_minor": {"x": 1}}"#,
    )
    .unwrap();
    assert!(swarm.herdr_ok && !swarm.inbox_ok);
    assert_eq!(swarm.stale_after, STALE_AFTER);
    let o = &swarm.roots[0];
    assert_eq!(o.herdr_status, HerdrStatus::Unknown);
    assert_eq!((o.harness.as_str(), o.hidden_finished), ("claude", 0));
    let a = &o.agents[0];
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
fn an_agent_without_an_id_or_a_root_without_its_pane_does_not_read() {
    assert!(serde_json::from_str::<Agent>(r#"{"role": "x"}"#).is_err());
    assert!(
        serde_json::from_str::<Root>(r#"{"tab_id": null, "tab_label": "x", "session_id": null}"#)
            .is_err()
    );
}
