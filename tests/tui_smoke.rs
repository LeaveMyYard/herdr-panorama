//! `panorama tui` end to end in a pseudo-terminal, with no herdr to talk to: it collects on
//! its own thread, draws the graph's "herdr is not reachable" page, writes no panic to its
//! log, and `q` ends it with exit status 0.

use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::io::{FdFlags, fcntl_getfd, fcntl_setfd};
use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{Winsize, tcsetwinsize};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn the_tui_draws_the_unreachable_page_and_q_ends_it() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("state");
    std::fs::create_dir_all(&state).unwrap();
    let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).unwrap();
    fcntl_setfd(&master, fcntl_getfd(&master).unwrap() | FdFlags::CLOEXEC).unwrap();
    grantpt(&master).unwrap();
    unlockpt(&master).unwrap();
    let name = ptsname(&master, Vec::new()).unwrap();
    let slave = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(name.to_str().unwrap())
        .unwrap();
    let size = Winsize {
        ws_row: 45,
        ws_col: 160,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    tcsetwinsize(&slave, size).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_panorama"))
        .arg("tui")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", dir.path())
        .env("TERM", "xterm-256color")
        .env("XDG_CONFIG_HOME", dir.path().join("cfg"))
        .env("XDG_STATE_HOME", dir.path().join("xdg-state"))
        .env("XDG_RUNTIME_DIR", dir.path())
        .env("HERDR_SOCKET_PATH", dir.path().join("no-herdr.sock"))
        .env("HERDR_BIN_PATH", dir.path().join("no-herdr"))
        .env("HERDR_PLUGIN_STATE_DIR", &state)
        .env("HERDR_INBOX_DB", dir.path().join("no-inbox.db"))
        .stdin(Stdio::from(slave.try_clone().unwrap()))
        .stdout(Stdio::from(slave.try_clone().unwrap()))
        .stderr(Stdio::from(slave))
        .spawn()
        .unwrap();
    let flags = fcntl_getfl(&master).unwrap();
    fcntl_setfl(&master, flags | OFlags::NONBLOCK).unwrap();
    let mut pty = std::fs::File::from(master);
    let (mut seen, mut buf) = (Vec::new(), [0u8; 65536]);
    let start = Instant::now();
    let drew = loop {
        match pty.read(&mut buf) {
            Ok(n) => seen.extend_from_slice(&buf[..n]),
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
        if String::from_utf8_lossy(&seen).contains("herdr is not reachable") {
            break true;
        }
        if start.elapsed() > Duration::from_secs(20) {
            break false;
        }
    };
    pty.write_all(b"q").unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        let _ = pty.read(&mut buf); // never let it wait on a full pty
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("q did not end the tui");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(drew, "no unreachable page in {} bytes", seen.len());
    assert_eq!(status.code(), Some(0));
    assert!(!state.join("tui.log").exists(), "a panic was logged");
}
