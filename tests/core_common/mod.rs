//! What the core's server tests share: a table pane check, the paths, a viewer's side of
//! the socket.
#![allow(dead_code)]

use panorama::core::{Config, Core, Event, Limits, NotStarted, Verify, events, start};
use serde_json::{Value, json};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

/// A pane check answering from a table, recording what it was asked.
#[derive(Clone, Default)]
pub struct Panes {
    pub ours: Vec<&'static str>,
    pub asked: Arc<Mutex<Vec<(String, i32)>>>,
    /// How long each check takes.
    pub slow: Duration,
}

impl Verify for Panes {
    async fn verify(&self, pane: &str, pid: i32) -> bool {
        self.asked.lock().unwrap().push((pane.to_owned(), pid));
        tokio::time::sleep(self.slow).await;
        self.ours.contains(&pane)
    }
}

/// This process's uid: the owner of a directory it just made.
pub fn uid() -> u32 {
    std::fs::metadata(tempfile::tempdir().unwrap().path())
        .unwrap()
        .uid()
}

pub fn paths(dir: &Path) -> (PathBuf, PathBuf) {
    (
        dir.join("s").join("core.sock"),
        dir.join("s").join("core.lock"),
    )
}

pub fn cfg(dir: &Path) -> Config {
    let (sock, lock) = paths(dir);
    Config::new(sock, lock, uid())
}

pub async fn up(c: Config, verify: Panes) -> (Core, mpsc::Receiver<Event>) {
    let (tx, rx) = events();
    (start(c, verify, tx).await.expect("the core starts"), rx)
}

pub async fn connect(sock: &Path) -> UnixStream {
    UnixStream::connect(sock).await.unwrap()
}

pub fn line(v: Value) -> Vec<u8> {
    let mut l = serde_json::to_vec(&v).unwrap();
    l.push(b'\n');
    l
}

pub async fn hello(s: &mut UnixStream, extra: Value) {
    let mut h = json!({"protocol": "1.3"});
    h.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    s.write_all(&line(json!({ "hello": h }))).await.unwrap();
}

/// The next `n` messages, read a byte at a time so nothing past them is taken off the socket
/// (a buffered reader per call would lose what it read ahead).
pub async fn lines(s: &mut UnixStream, n: usize) -> Vec<Value> {
    let mut out = vec![];
    while out.len() < n {
        let mut l = vec![];
        loop {
            let mut b = [0u8; 1];
            let got = tokio::time::timeout(Duration::from_secs(5), s.read(&mut b))
                .await
                .expect("a line in time")
                .unwrap();
            assert!(got > 0, "closed after {} lines", out.len());
            if b[0] == b'\n' {
                break;
            }
            l.push(b[0]);
        }
        out.push(serde_json::from_slice(&l).unwrap());
    }
    out
}

/// `start` on a thread of its own with a hard deadline: a start that blocks (on the lock, say)
/// fails the test instead of hanging the suite.
pub fn start_within(c: Config, secs: u64) -> Result<(), NotStarted> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (etx, _erx) = events();
        let _ = tx.send(rt.block_on(start(c, Panes::default(), etx)).map(drop));
    });
    rx.recv_timeout(Duration::from_secs(secs))
        .expect("start returned in time")
}

/// The connection ends without another word.
pub async fn closed(s: &mut UnixStream) -> bool {
    let mut b = [0u8; 100];
    matches!(
        tokio::time::timeout(Duration::from_secs(5), s.read(&mut b)).await,
        Ok(Ok(0)) | Ok(Err(_))
    )
}

pub async fn wait(mut f: impl FnMut() -> bool) -> bool {
    for _ in 0..200 {
        if f() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}

pub fn swarm_state() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/inputs/swarm-design9.json"
    );
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    json!({"state": {"protocol": "1.3", "swarm": fixture["swarm"], "unseen": {"a5": "done"},
        "focused_pane": "w1:p3", "visible_tabs": ["w1:t3"], "panels": {}}})
}

pub fn quick(dir: &Path, limits: Limits) -> Config {
    Config { limits, ..cfg(dir) }
}
