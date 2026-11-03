use panorama::host::command;
use std::{
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[test]
fn cancelling_a_blocked_owned_read_reaps_only_its_helper() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("pid");
    let path = marker.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let task = std::thread::spawn(move || {
        let mut child = Command::new("python3");
        child.arg("-c").arg("import pathlib,os,sys,time;pathlib.Path(sys.argv[1]).write_text(str(os.getpid()));time.sleep(30)").arg(path);
        command::read(child, Duration::from_secs(5), 4096, Some(&flag))
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    while !marker.exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let pid = std::fs::read_to_string(marker)
        .unwrap()
        .parse::<u32>()
        .unwrap();
    let identity = herdr_agent_kit::agent_procs::start_token(pid).unwrap();
    let started = Instant::now();
    stop.store(true, Ordering::Relaxed);
    assert_eq!(task.join().unwrap().unwrap_err(), "helper read cancelled");
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_ne!(
        herdr_agent_kit::agent_procs::start_token(pid).as_ref(),
        Some(&identity)
    );
}
