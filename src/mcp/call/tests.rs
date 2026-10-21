use super::*;
use serde_json::json;

fn args(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

fn call(v: Value, sid: Option<&str>, dir: &Path) -> Value {
    let r = arguments(&args(v)).unwrap();
    serde_json::from_str(&crate::snapshot::pyjson::dumps(
        &report_thread(r, sid, dir, 1_791_960_000.0),
        2,
    ))
    .unwrap()
}

#[test]
fn tool_uses_the_session_it_is_given() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        call(json!({"thread_id": "t1"}), None, dir.path())["status"],
        "unavailable"
    );
    let out = call(
        json!({"thread_id": "t1", "title": "Task 1\u{1b}[2J", "phase": "implement", "next": ["review"]}),
        Some("s2"),
        dir.path(),
    );
    assert_eq!(out["status"], "recorded");
    assert_eq!(
        out["phases"],
        json!([["implement", "now"], ["review", "next"]])
    );
    let d = crate::report::merge::current(&events_path(dir.path(), "s2"), "t1").unwrap();
    assert_eq!(d.name.as_deref(), Some("Task 1[2J")); // control characters stripped
    assert_eq!(
        call(
            json!({"thread_id": "t1", "status": "bogus"}),
            Some("s2"),
            dir.path()
        )["status"],
        "error"
    );
}

#[test]
fn m1_limits_and_no_wildcard_claims() {
    let dir = tempfile::tempdir().unwrap();
    let s = Some("s4");
    for bad in [json!(["*"]), json!(["ab*"])] {
        assert_eq!(
            call(json!({"thread_id": "t", "agents": bad}), s, dir.path())["status"],
            "error"
        );
    }
    assert_eq!(
        call(
            json!({"thread_id": "t", "agents": ["Review PR*"]}),
            s,
            dir.path()
        )["status"],
        "recorded"
    );
    let many: Vec<String> = (0..100).map(|i| format!("p{i}")).collect();
    assert_eq!(
        call(
            json!({"thread_id": "t", "phases_done": many, "next": ["n"]}),
            s,
            dir.path()
        )["status"],
        "error"
    );
    call(
        json!({"thread_id": "t", "title": "x\u{202e}evil\u{2066}", "phase": "a".repeat(500)}),
        s,
        dir.path(),
    );
    let d = crate::report::merge::current(&events_path(dir.path(), "s4"), "t").unwrap();
    assert_eq!(d.name.as_deref(), Some("xevil"));
    assert!(d.phases[0]["name"].as_str().unwrap().chars().count() <= 200);
}

#[test]
fn m1_the_events_file_is_compacted() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..400 {
        call(
            json!({"thread_id": "t", "title": "Thread", "phase": format!("phase {}", i % 7), "next": ["review", "merge"]}),
            Some("s5"),
            dir.path(),
        );
    }
    let path = events_path(dir.path(), "s5");
    assert!(std::fs::metadata(&path).unwrap().len() < 2 * crate::report::COMPACT_AT);
    let d = crate::report::merge::current(&path, "t").unwrap();
    assert_eq!(d.name.as_deref(), Some("Thread"));
    assert_eq!(d.phases.last().unwrap()["name"], "merge");
}

#[test]
fn the_schema_s_types_are_checked() {
    assert_eq!(
        arguments(&args(json!({"title": "no id"}))).unwrap_err(),
        "thread_id: Field required"
    );
    assert!(arguments(&args(json!({"thread_id": 3}))).is_err());
    assert!(arguments(&args(json!({"thread_id": "t", "next": "x"}))).is_err());
    assert!(arguments(&args(json!({"thread_id": "t", "next": [1]}))).is_err());
    let r = arguments(&args(
        json!({"thread_id": "t", "title": null, "agents": ["a"]}),
    ))
    .unwrap();
    assert_eq!((r.title, r.agents), (None, Some(vec!["a".to_owned()])));
    assert_eq!(py_repr("ab*"), "'ab*'");
    assert_eq!(py_repr("it's"), "\"it's\"");
}
