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
