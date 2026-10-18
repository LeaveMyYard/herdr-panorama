use super::*;

#[test]
fn a_first_read_starts_ten_minutes_before_the_window() {
    assert_eq!(since(10_000.0, 3600.0), 10_000.0 - 3600.0 - 600.0);
}

#[test]
fn an_id_that_is_not_a_session_is_never_looked_up() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Sessions::new(dir.path().to_path_buf(), dir.path().join("hooks"));
    for bad in ["../x", "*", "", "/etc/passwd"] {
        assert!(s.session(bad, None, 0.0).is_none(), "{bad}");
    }
    assert_eq!(
        s.live_session("../x", None, 0.0),
        (vec!["../x".to_owned()], None)
    );
    assert!(s.continuations(&["../x".to_owned()]).is_empty());
}

#[test]
fn continuations_never_name_the_sessions_asked_about() {
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("p");
    std::fs::create_dir_all(&proj).unwrap();
    let (a, b, c) = (
        "aaaaaaaa-0000-4000-8000-000000000001",
        "bbbbbbbb-0000-4000-8000-000000000002",
        "cccccccc-0000-4000-8000-000000000003",
    );
    let line = |from: &str, to: &str| {
        format!(
            "{{\"type\":\"continued-in\",\"sessionId\":\"{from}\",\"continuedInSessionId\":\"{to}\"}}\n"
        )
    };
    std::fs::write(proj.join(format!("{a}.jsonl")), line(a, b)).unwrap();
    std::fs::write(proj.join(format!("{b}.jsonl")), line(b, c)).unwrap();
    let mut s = Sessions::new(dir.path().to_path_buf(), dir.path().join("hooks"));
    for id in [a, b] {
        assert!(s.session(id, None, 0.0).is_some());
    }
    assert_eq!(s.continuations(&[a.to_owned()]), [b, c]);
    assert_eq!(s.continuations(&[a.to_owned(), b.to_owned()]), [c]); // b is asked about: not a continuation
}
