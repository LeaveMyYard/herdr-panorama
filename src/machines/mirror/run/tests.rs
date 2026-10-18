use super::*;
use std::path::Path;
use std::time::Instant;

fn sh(script: &str) -> Vec<String> {
    vec!["sh".into(), "-c".into(), script.into()]
}

fn scratch(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new().prefix(name).tempdir().unwrap()
}

fn alive(pid: i32) -> bool {
    let pid = rustix::process::Pid::from_raw(pid).unwrap();
    rustix::process::test_kill_process(pid).is_ok()
}

#[test]
fn a_timeout_kills_rsync_and_its_children() {
    let dir = scratch("run-timeout");
    let pidfile = dir.path().join("child");
    let start = Instant::now();
    let script = format!("sleep 30 & echo $! > {}; wait", pidfile.display());
    let err = run(&sh(&script), Duration::from_millis(500));
    assert_eq!(err.as_deref(), Some(TIMED_OUT));
    assert!(start.elapsed() < Duration::from_secs(3));
    let child: i32 = std::fs::read_to_string(&pidfile)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let gone = Instant::now() + Duration::from_secs(2);
    while alive(child) && Instant::now() < gone {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!alive(child), "the backgrounded child went with the group");
}

#[test]
fn success_and_vanished_files_are_no_error() {
    assert_eq!(run(&sh("exit 0"), Duration::from_secs(5)), None);
    assert_eq!(
        run(&sh("echo gone >&2; exit 24"), Duration::from_secs(5)),
        None
    );
}

#[test]
fn a_failure_says_why() {
    let t = Duration::from_secs(5);
    assert_eq!(
        run(&sh("echo '  no route  ' >&2; exit 12"), t).as_deref(),
        Some("no route")
    );
    assert_eq!(run(&sh("exit 3"), t).as_deref(), Some("rsync exit 3"));
    let long = run(&sh("printf '%0400d' 0 >&2; exit 1"), t).unwrap();
    assert_eq!(long.chars().count(), 300);
    let missing = run(&["/nonexistent/rsync".to_owned()], t).unwrap();
    assert!(missing.starts_with("rsync: "), "{missing}");
    assert!(!Path::new("/nonexistent/rsync").exists());
}

#[test]
fn a_run_ended_by_a_signal_reads_as_python_returncode() {
    let err = run(&sh("kill -TERM $$"), Duration::from_secs(5));
    assert_eq!(err.as_deref(), Some("rsync exit -15"));
}
