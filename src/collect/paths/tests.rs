use super::*;

fn env(vars: &[(&str, &str)]) -> PluginEnv {
    PluginEnv::from_vars(vars.iter().copied(), Some("/home/me".into()), 1000)
}

#[test]
fn the_dirs_follow_their_variables_then_the_state_home() {
    assert_eq!(
        events_dir(&env(&[])),
        Path::new("/home/me/.local/state/herdr-panorama/events")
    );
    let e = env(&[("XDG_STATE_HOME", "/s"), ("PANORAMA_HOOKS_DIR", "/h")]);
    assert_eq!(events_dir(&e), Path::new("/s/herdr-panorama/events"));
    assert_eq!(hooks_dir(&e), Path::new("/h"));
    // the plugin state dir is not where these live
    let e = env(&[("HERDR_PLUGIN_STATE_DIR", "/p")]);
    assert_eq!(
        hooks_dir(&e),
        Path::new("/home/me/.local/state/herdr-panorama/hooks")
    );
}

#[test]
fn file_names_are_safe() {
    let id = "0E4A6B1C-9D2F-4A3B-8C7D-1E2F3A4B5C6D";
    assert_eq!(
        events_path(Path::new("/e"), id),
        Path::new("/e/0e4a6b1c-9d2f-4a3b-8c7d-1e2f3a4b5c6d.jsonl")
    );
    assert_eq!(
        events_path(Path::new("/e"), "../x y"),
        Path::new("/e/xy.jsonl")
    );
    assert_eq!(
        hooks_path(Path::new("/h"), id),
        Path::new(&format!("/h/{id}.jsonl"))
    );
}

#[test]
fn an_events_file_is_named_by_the_canonical_id() {
    // tests/test_bridge_rules.py: an id in any case names one file; any other id keeps its safe
    // characters
    let hex = "0e4a6b1c-9d2f-4a3b-8c7d-1e2f3a4b5c6d";
    let dir = Path::new("/e");
    assert_eq!(events_path(dir, &hex.to_uppercase()), events_path(dir, hex));
    assert_eq!(events_path(dir, hex), dir.join(format!("{hex}.jsonl")));
    assert_eq!(events_path(dir, "not/a-uuid"), dir.join("nota-uuid.jsonl"));
}
