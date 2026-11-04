use super::*;

fn env(vars: &[(&str, &str)]) -> PluginEnv {
    PluginEnv::from_vars(vars.iter().copied(), Some("/home/me".into()), 1000)
}

#[test]
fn the_dirs_follow_their_variables_then_the_state_home() {
    assert_eq!(
        hooks_dir(&env(&[("XDG_STATE_HOME", "/s")])),
        Path::new("/s/herdr-panorama/hooks")
    );
    let e = env(&[("XDG_STATE_HOME", "/s"), ("PANORAMA_HOOKS_DIR", "/h")]);
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
        hooks_path(Path::new("/h"), "../x y"),
        Path::new("/h/xy.jsonl")
    );
    assert_eq!(
        hooks_path(Path::new("/h"), id),
        Path::new(&format!("/h/{id}.jsonl"))
    );
}
