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
    let err = run(&sh(&script), Duration::from_millis(500), &|| false);
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
    assert_eq!(run(&sh("exit 0"), Duration::from_secs(5), &|| false), None);
    assert_eq!(
        run(
            &sh("echo gone >&2; exit 24"),
            Duration::from_secs(5),
            &|| false
        ),
        None
    );
}

#[test]
fn a_failure_says_why() {
    let t = Duration::from_secs(5);
    assert_eq!(
        run(&sh("echo '  no route  ' >&2; exit 12"), t, &|| false).as_deref(),
        Some("no route")
    );
    assert_eq!(
        run(&sh("exit 3"), t, &|| false).as_deref(),
        Some("rsync exit 3")
    );
    let long = run(&sh("printf '%0400d' 0 >&2; exit 1"), t, &|| false).unwrap();
    assert_eq!(long.chars().count(), 300);
    let missing = run(&["/nonexistent/rsync".to_owned()], t, &|| false).unwrap();
    assert!(missing.starts_with("rsync: "), "{missing}");
    assert!(!Path::new("/nonexistent/rsync").exists());
}

#[test]
fn a_run_ended_by_a_signal_reads_as_python_returncode() {
    let err = run(&sh("kill -TERM $$"), Duration::from_secs(5), &|| false);
    assert_eq!(err.as_deref(), Some("rsync exit -15"));
}

/// The real thing, where the OS produces it: Linux returns ETXTBSY for a file open for
/// writing; macOS does not (the bound itself is tested in `bounded/tests.rs` everywhere).
#[cfg(target_os = "linux")]
#[test]
fn a_binary_held_open_for_writing_fails_fast_and_runs_once_released() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("run-busy");
    let bin = dir.path().join("rsync");
    std::fs::write(&bin, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    let argv = [bin.display().to_string()];
    let held = std::fs::OpenOptions::new().write(true).open(&bin).unwrap(); // busy: ETXTBSY
    // on a thread: without the bound the retries would never end, and this test must fail,
    // not hang
    let (tx, rx) = std::sync::mpsc::channel();
    let busy = argv.clone();
    std::thread::spawn(move || tx.send(run(&busy, Duration::from_secs(5), &|| false)));
    let err = rx
        .recv_timeout(Duration::from_secs(1))
        .expect("the retries end within a second");
    let err = err.unwrap();
    assert!(err.contains("Text file busy"), "{err}");
    drop(held);
    assert_eq!(run(&argv, Duration::from_secs(5), &|| false), None);
}

#[test]
fn a_cancelled_run_kills_its_own_group_at_once() {
    let dir = scratch("run-cancel");
    let pidfile = dir.path().join("child");
    let script = format!("sleep 30 & echo $! > {}; wait", pidfile.display());
    let since = Instant::now();
    let stop = || since.elapsed() > Duration::from_millis(300) && pidfile.exists();
    let start = Instant::now();
    assert_eq!(
        run(&sh(&script), Duration::from_secs(30), &stop).as_deref(),
        Some(STOPPED)
    );
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "{:?}",
        start.elapsed()
    );
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
