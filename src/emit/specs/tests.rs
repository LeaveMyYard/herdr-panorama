use super::*;
use crate::snapshot::pyjson::dumps_line;

#[test]
fn phase_specs_read_as_python_partitions_them() {
    // the answers Python's `str.partition` chain gives (a `:` in a time ends the time: parity)
    let p = |s: &str| dumps_line(&phase(s));
    assert_eq!(p("build"), r#"{"name": "build", "state": "next"}"#);
    assert_eq!(p("build=now"), r#"{"name": "build", "state": "now"}"#);
    assert_eq!(
        p("build=done@09:30-10:15:shipped"),
        r#"{"name": "build", "state": "done", "start": "09", "note": "30-10:15:shipped"}"#
    );
    assert_eq!(
        p("review=now@2026-10-18T09:00:00Z"),
        r#"{"name": "review", "state": "now", "start": "2026", "end": "10-18T09", "note": "00:00Z"}"#
    );
    assert_eq!(
        p("x=:a note"),
        r#"{"name": "x", "state": "next", "note": "a note"}"#
    );
    assert_eq!(
        p("b=done@0930-1015"),
        r#"{"name": "b", "state": "done", "start": "0930", "end": "1015"}"#
    );
}

#[test]
fn agent_specs_read_as_python_partitions_them() {
    let a = |s: &str| dumps_line(&agent(s));
    assert_eq!(a("a1b2"), r#"{"match": "a1b2"}"#);
    assert_eq!(
        a("Review PR*=reviewer@2"),
        r#"{"match": "Review PR*", "role": "reviewer", "phase": 2}"#
    );
    assert_eq!(a("x=@two"), r#"{"match": "x"}"#);
    assert_eq!(a("x=writer@-1"), r#"{"match": "x", "role": "writer"}"#);
}
