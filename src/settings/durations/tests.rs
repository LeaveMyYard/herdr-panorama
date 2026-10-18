use super::*;
use serde_json::json;

fn cfg(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

#[test]
fn durations_read_as_python_parse_duration_reads_them() {
    let d = |v: Value| parse_duration(&v);
    assert_eq!(d(json!(90)), Some(90.0));
    assert_eq!(d(json!(1.5)), Some(1.5));
    assert_eq!(d(json!(0)), Some(0.0));
    assert_eq!(d(json!(-1)), None);
    assert_eq!(d(json!(true)), None);
    assert_eq!(d(json!(" 42 ")), Some(42.0));
    assert_eq!(d(json!("90m")), Some(5400.0));
    assert_eq!(d(json!("1H")), Some(3600.0));
    assert_eq!(d(json!("2h30m")), Some(9000.0));
    assert_eq!(d(json!(" 2 h 30 m 5 s ")), Some(9005.0));
    assert_eq!(d(json!("1h5s")), Some(3605.0));
    assert_eq!(d(json!("45s")), Some(45.0));
    for bad in ["", "  ", "h", "1h30", "30m1h", "1x", "1.5h", "-5", "1hh"] {
        assert_eq!(d(json!(bad)), None, "{bad:?}");
    }
    assert_eq!(d(json!(null)), None);
    assert_eq!(d(json!([1])), None);
}

#[test]
fn the_environment_beats_the_file_and_a_bad_value_takes_the_next() {
    let file = cfg(json!({"done_window": "2h", "stale_after": 60}));
    assert_eq!(done_window(None, &file), 7200.0);
    assert_eq!(done_window(Some("90"), &file), 90.0);
    assert_eq!(done_window(Some("soon"), &file), 7200.0);
    assert_eq!(done_window(Some(""), &file), 7200.0);
    assert_eq!(done_window(None, &cfg(json!({}))), DEFAULT_DONE_WINDOW);
    assert_eq!(
        done_window(None, &cfg(json!({"done_window": "x"}))),
        DEFAULT_DONE_WINDOW
    );
    assert_eq!(stale_after(None, &file), 60.0);
    assert_eq!(stale_after(Some("1m"), &file), 60.0);
    assert_eq!(stale_after(None, &cfg(json!({}))), DEFAULT_STALE_AFTER);
}

fn read(file: &mut herdr_agent_kit::plugin_env::ConfigFile) -> Map<String, Value> {
    file.load_as().unwrap_or_default()
}

#[test]
fn default_is_one_hour() {
    let dir = tempfile::tempdir().unwrap();
    let mut file = herdr_agent_kit::plugin_env::ConfigFile::in_dir(dir.path());
    assert_eq!(done_window(None, &read(&mut file)), 3600.0);
}

#[test]
fn config_file_and_env_override() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let mut file = herdr_agent_kit::plugin_env::ConfigFile::in_dir(dir.path());
    std::fs::write(&path, "done_window = \"2h30m\"\n").unwrap();
    assert_eq!(done_window(None, &read(&mut file)), 9000.0);
    std::fs::write(&path, "done_window = 600\n").unwrap(); // a new size: re-read
    assert_eq!(done_window(None, &read(&mut file)), 600.0);
    assert_eq!(done_window(Some("45m"), &read(&mut file)), 2700.0);
}

#[test]
fn bad_values_fall_back() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("config.toml"), "done_window = [\n").unwrap(); // not TOML
    let mut file = herdr_agent_kit::plugin_env::ConfigFile::in_dir(dir.path());
    assert_eq!(done_window(None, &read(&mut file)), 3600.0);
    assert_eq!(parse_duration(&json!("soon")), None);
    assert_eq!(parse_duration(&json!(-5)), None);
    assert_eq!(parse_duration(&json!("90m")), Some(5400.0));
    assert_eq!(parse_duration(&json!("15")), Some(15.0));
}
