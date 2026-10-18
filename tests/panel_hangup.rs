//! #77 H1: closing the panel's pane (its terminal goes away) is the operator closing it: the
//! panel exits 0 and leaves no crash marker (the core would read a crash).

use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{Winsize, tcsetwinsize};
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn a_terminal_that_goes_away_is_a_clean_end() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("state");
    std::fs::create_dir_all(&state).unwrap();
    // CLOEXEC: a master the panel inherited would keep its terminal alive
    let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY | OpenptFlags::CLOEXEC).unwrap();
    grantpt(&master).unwrap();
    unlockpt(&master).unwrap();
    let name = ptsname(&master, Vec::new()).unwrap();
    let slave = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(name.to_str().unwrap())
        .unwrap();
    tcsetwinsize(
        &slave,
        Winsize {
            ws_row: 45,
            ws_col: 46,
            ws_xpixel: 0,
            ws_ypixel: 0,
        },
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_panorama"))
        .arg("panel")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", dir.path())
        .env("TERM", "xterm-256color")
        .env("XDG_CONFIG_HOME", dir.path().join("cfg"))
        .env("XDG_RUNTIME_DIR", dir.path())
        .env("HERDR_SOCKET_PATH", dir.path().join("no-herdr.sock"))
        .env("HERDR_PLUGIN_STATE_DIR", &state)
        .env("HERDR_PANE_ID", "w9:p9")
        .env("HERDR_TAB_ID", "w9:t9")
        .stdin(Stdio::from(slave.try_clone().unwrap()))
        .stdout(Stdio::from(slave.try_clone().unwrap()))
        .stderr(Stdio::from(slave))
        .spawn()
        .unwrap();
    // let it start and draw (reading what it draws, so it never waits on a full pty)
    let flags = fcntl_getfl(&master).unwrap();
    fcntl_setfl(&master, flags | OFlags::NONBLOCK).unwrap();
    let mut out = std::fs::File::from(master);
    let (mut seen, mut buf) = (0, [0u8; 65536]);
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(1500) {
        match out.read(&mut buf) {
            Ok(n) => seen += n,
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    assert!(seen > 0, "the panel drew nothing");
    drop(out); // the pane closes
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("the panel outlived its terminal");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let marks: Vec<_> = std::fs::read_dir(&state)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("panel-crash"))
        .collect();
    assert_eq!(status.code(), Some(0), "markers: {marks:?}");
    assert!(marks.is_empty(), "{marks:?}");
}
