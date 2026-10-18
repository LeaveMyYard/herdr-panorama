use super::*;
use std::os::unix::fs::PermissionsExt;
use std::time::Instant;

fn fake_rsync(dir: &std::path::Path, body: &str) -> String {
    let path = dir.join("rsync");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.display().to_string()
}

#[test]
fn the_help_text_decides() {
    let dir = tempfile::tempdir().unwrap();
    let t = Duration::from_secs(5);
    assert!(knows_append_verify(
        &fake_rsync(dir.path(), "echo ' --append-verify  like --append'"),
        t
    ));
    assert!(knows_append_verify(
        &fake_rsync(dir.path(), "echo '--append-verify' >&2; exit 1"),
        t
    ));
    assert!(!knows_append_verify(
        &fake_rsync(dir.path(), "echo openrsync"),
        t
    ));
    assert!(!knows_append_verify("/nonexistent/rsync", t));
}

#[test]
fn a_hung_help_is_killed_at_its_deadline() {
    let dir = tempfile::tempdir().unwrap();
    let pidfile = dir.path().join("pid");
    let bin = fake_rsync(
        dir.path(),
        &format!("echo $$ > {}; exec sleep 30", pidfile.display()),
    );
    let start = Instant::now();
    assert!(!knows_append_verify(&bin, Duration::from_millis(300)));
    assert!(start.elapsed() < Duration::from_secs(3));
    let pid: i32 = std::fs::read_to_string(&pidfile)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let pid = rustix::process::Pid::from_raw(pid).unwrap();
    let until = Instant::now() + Duration::from_secs(2);
    while rustix::process::test_kill_process(pid).is_ok() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        rustix::process::test_kill_process(pid).is_err(),
        "the hung help is gone"
    );
}
