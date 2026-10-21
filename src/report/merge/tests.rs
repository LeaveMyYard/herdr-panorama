use super::*;

const T0: f64 = 1_791_960_000.0;

fn r(thread: &str) -> Report {
    Report {
        thread: thread.into(),
        ..Report::default()
    }
}

fn strs(v: &[&str]) -> Option<Vec<String>> {
    Some(v.iter().map(|s| (*s).to_owned()).collect())
}

fn field<'a>(d: &'a Declared, name: &str, key: &str) -> Option<&'a str> {
    let p = d.phases.iter().find(|p| p["name"] == name)?;
    p.get(key).and_then(Value::as_str)
}

#[test]
fn report_merges_phases_and_keeps_times() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s1.jsonl");
    let first = Report {
        title: Some("PR #7".into()),
        phase: Some("author".into()),
        next: strs(&["review r1", "merge"]),
        ..r("pr-7")
    };
    report(&path, &first, T0).unwrap();
    let second = Report {
        phase: Some("review r1".into()),
        phases_done: strs(&["author"]),
        ..r("pr-7")
    };
    let out = report(&path, &second, T0 + 600.0).unwrap();
    assert_eq!(out.thread_status, None); // the thread already has a status
    let d = current(&path, "pr-7").unwrap();
    assert_eq!(
        (d.name.as_deref(), d.status.as_deref()),
        (Some("PR #7"), Some("active"))
    );
    let names: Vec<_> = d
        .phases
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["author", "review r1", "merge"]);
    assert_eq!(field(&d, "author", "state"), Some("done"));
    assert_eq!(field(&d, "author", "start"), Some(stamp(T0).as_str()));
    assert_eq!(field(&d, "author", "end"), Some(stamp(T0 + 600.0).as_str()));
    assert_eq!(
        (field(&d, "review r1", "state"), field(&d, "merge", "state")),
        (Some("now"), Some("next"))
    );
    let last = Report {
        phase: Some("merge".into()),
        phases_done: strs(&["author", "review r1"]),
        status: Some("done".into()),
        ..r("pr-7")
    };
    report(&path, &last, T0 + 900.0).unwrap();
    let d = current(&path, "pr-7").unwrap();
    assert_eq!(d.status.as_deref(), Some("done"));
    assert!(d.phases.iter().all(|p| p["state"] == "done"));
    assert_eq!(field(&d, "author", "end"), Some(stamp(T0 + 600.0).as_str())); // kept
}

#[test]
fn the_results_say_what_was_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.jsonl");
    let full = Report {
        title: Some("PR #595 ledger".into()),
        phase: Some("review r1".into()),
        phases_done: strs(&["author"]),
        next: strs(&["merge"]),
        agents: strs(&["review*", "a1b2c3"]),
        ..r("pr-595")
    };
    let out = report(&path, &full, T0).unwrap();
    let pairs = |o: &Reported| {
        o.phases
            .iter()
            .map(|(a, b)| format!("{a}={b}"))
            .collect::<Vec<_>>()
    };
    assert_eq!(pairs(&out), ["author=done", "review r1=now", "merge=next"]);
    assert_eq!(out.thread_status.as_deref(), Some("active"));
    let out = report(
        &path,
        &Report {
            phase: Some("merge".into()),
            phases_done: strs(&["author", "review r1"]),
            ..r("pr-595")
        },
        T0,
    )
    .unwrap();
    assert_eq!(pairs(&out), ["author=done", "review r1=done", "merge=now"]);
    let out = report(
        &path,
        &Report {
            status: Some("done".into()),
            ..r("pr-595")
        },
        T0,
    )
    .unwrap();
    assert_eq!(pairs(&out), ["author=done", "review r1=done"]); // `merge` was now, not done or next
    assert_eq!(out.thread_status.as_deref(), Some("done"));
    let d = current(&path, "pr-595").unwrap();
    let agents: Vec<_> = d
        .agents
        .iter()
        .map(|a| a["match"].as_str().unwrap())
        .collect();
    assert_eq!(agents, ["review*", "a1b2c3"]);
}
