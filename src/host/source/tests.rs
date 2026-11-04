use super::*;
use crate::host::link::Linker;

fn list() -> Value {
    serde_json::from_str(include_str!("../../../tests/fixtures/hc-list.json")).unwrap()
}

#[test]
fn recorded_hc_cli_shape_maps_only_supported_roots_without_membership_guesses() {
    let rows = normalize(list()).unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0]["pane_id"], "core:t2");
    assert_eq!(rows[1]["agent"], "claude");
    assert_eq!(rows[1]["agent_status"], "blocked");
    assert_eq!(rows[2]["agent_status"], "done");
    assert_eq!(rows[3]["terminal"], "vps/t7");
    for row in rows {
        assert!(row["tab_id"].is_null() && row["workspace_id"].is_null());
        assert_eq!(row["focused"], false);
        assert!(row.get("agent_session").is_none());
        assert!(row.get("where").is_none());
    }
}

#[test]
fn malformed_source_is_an_error_and_unknown_status_stays_unknown() {
    assert!(normalize(json!([])).is_err());
    let mut doc = list();
    doc["terminals"][1]["address"] = json!("t2");
    assert!(normalize(doc).is_err());
    let mut doc = list();
    doc["terminals"][0]["status"] = json!({});
    assert!(normalize(doc).is_err());
    let mut doc = list();
    doc["terminals"][0]["status"] = json!("future-word");
    assert_eq!(normalize(doc).unwrap()[0]["agent_status"], "future-word");
}

#[test]
fn missing_identity_keeps_plain_roots_and_exposes_availability_errors() {
    use std::{os::unix::fs::PermissionsExt, time::Duration};
    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("hc");
    let wire = list().to_string();
    std::fs::write(dir.path().join("list.json"), wire).unwrap();
    std::fs::write(&binary,"#!/usr/bin/env python3\nimport sys,pathlib\nassert sys.argv[1:]==['ls','--json']\nprint(pathlib.Path(__file__).with_name('list.json').read_text())\n").unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let env = PluginEnv::from_vars(
        Vec::<(String, String)>::new(),
        Some(dir.path().into()),
        1000,
    );
    let link = Linker::system(&env);
    let mut source = CoreSource {
        link,
        core: Core::new(binary.into_os_string(), Duration::from_secs(1)),
        env,
        hooks_dir: dir.path().join("no-hooks"),
    };
    let observation = source.read();
    assert_eq!(
        observation.snapshot.unwrap()["agents"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(observation.errors.len(), 4);
    assert_eq!(observation.terminals["core:vps/t7"], "vps/t7");
}
