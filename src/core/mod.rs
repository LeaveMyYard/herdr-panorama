//! The per-tab panels' core, server side (Python `core.py`; `docs/protocol/panel-v1.md`): it
//! publishes panorama's state to the panel viewers on a local socket. One core per herdr
//! session.
//!
//! Hardening, as Python's (the design review of #29, M1 and M7):
//! - one core: an exclusive flock on the core lock for the core's whole life ([`start`]);
//! - a socket file left by a dead core is removed only after a connect to it failed, lock held;
//! - the socket is 0600 in a directory that must be a real 0700 directory of ours, and a peer
//!   must have our uid;
//! - a connection must say hello within [`Limits::hello`], at most [`Limits::max_viewers`] are
//!   open at once, and a send that does not finish in [`Limits::send`] drops the viewer;
//! - lines over the protocol's 4 MiB close the connection;
//! - each viewer has a small bounded outbox ([`outbox`]): a slow viewer loses older `state`
//!   messages, it never stalls the core;
//! - a viewer's `hello.pane_id` is only a claim: [`Verify`] decides whether it is trusted, and
//!   it is never authority to close anything.
//!
//! What the viewers say reaches the app as [`Event`]s; the app answers the open key's request.

mod conn;
mod handle;
pub mod outbox;
mod start;
mod viewer;

pub use start::NotStarted;
pub use viewer::Role;

use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use viewer::Viewer;

/// The limits (Python `HELLO_SECONDS`, `SEND_SECONDS`, `PING_SECONDS`, `MAX_VIEWERS`, `OUTBOX`).
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub hello: Duration,
    pub send: Duration,
    pub ping: Duration,
    pub max_viewers: usize,
    pub outbox: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            hello: Duration::from_secs(5),
            send: Duration::from_secs(10),
            ping: Duration::from_secs(5),
            max_viewers: 64,
            outbox: 4,
        }
    }
}

/// Where the core listens and who it serves.
#[derive(Clone, Debug)]
pub struct Config {
    pub sock: PathBuf,
    pub lock: PathBuf,
    /// Ours: the socket's directory must belong to it.
    pub uid: u32,
    /// The uid a peer must have: ours (another value only in tests).
    pub peer_uid: u32,
    pub limits: Limits,
}

impl Config {
    /// The core at `sock` with its lock, for `uid`, with the protocol's limits.
    pub fn new(sock: PathBuf, lock: PathBuf, uid: u32) -> Self {
        Self {
            sock,
            lock,
            uid,
            peer_uid: uid,
            limits: Limits::default(),
        }
    }
}

/// What the viewers say, for the app.
#[derive(Debug)]
pub enum Event {
    /// The operator looked at these agents (one event per read of a connection).
    Seen(Vec<String>),
    /// A verified panel is about to put the keyboard on this pane (1.2).
    Focusing(String),
    /// A panel chose this filter (1.3); the app checks the name.
    Filter(String),
    /// The open key's request (1.1): the app answers what it did ("panel", "home", …).
    Open {
        req: Map<String, Value>,
        reply: oneshot::Sender<String>,
    },
    /// Something the app should log (Python's warnings).
    Note(String),
}

/// Whether a viewer's claimed pane is its own: herdr says that pane's foreground runs this
/// very process (`pid`) as `panorama panel` (Python `verify_pane`).
pub trait Verify: Send + Sync + 'static {
    fn verify(&self, pane: &str, pid: i32) -> impl Future<Output = bool> + Send;
}

/// A connected viewer, as the app sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connected {
    pub pane_id: Option<String>,
    pub tab_id: Option<String>,
    pub role: Role,
}

/// What every task of the core shares.
struct Shared {
    limits: Limits,
    uid: u32,
    viewers: Mutex<Vec<Arc<Viewer>>>,
    /// The last state, for viewers that connect later.
    last: Mutex<Option<Arc<Vec<u8>>>>,
    events: mpsc::UnboundedSender<Event>,
    refusing: std::sync::atomic::AtomicBool,
}

/// The running core. Dropping it (or [`Core::stop`]) closes every viewer, removes the socket
/// and releases the lock.
pub struct Core {
    shared: Arc<Shared>,
    sock: PathBuf,
    tasks: Vec<JoinHandle<()>>,
    _lock: herdr_agent_kit::plugin_env::lock::HeldLock,
}

impl Core {
    /// Sends `msg` to every viewer that said hello as a panel (a `state` is kept for later ones).
    pub fn publish(&self, msg: &Value) {
        let Ok(line) = crate::proto::framing::encode(msg) else {
            let _ = self.shared.events.send(Event::Note(
                "a message over the line limit was not sent".into(),
            ));
            return;
        };
        let line = Arc::new(line);
        if msg.get("state").is_some() {
            *self.shared.last.lock().unwrap() = Some(line.clone());
        }
        for v in self.shared.viewers.lock().unwrap().iter() {
            if v.wants_state() {
                v.send(line.clone());
            }
        }
    }

    /// The panels go (quit, per-tab off): every viewer is told and exits. Waits up to `wait`
    /// for the writers to take it, so it is said before the core stops.
    pub async fn shutdown(&self, wait: Duration) {
        let line =
            Arc::new(crate::proto::framing::encode(&serde_json::json!({"shutdown": {}})).unwrap());
        let told: Vec<Arc<Viewer>> = self
            .shared
            .viewers
            .lock()
            .unwrap()
            .iter()
            .filter(|v| v.is_panel())
            .cloned()
            .collect();
        for v in &told {
            v.tell_to_go(line.clone());
        }
        let end = tokio::time::Instant::now() + wait;
        while tokio::time::Instant::now() < end && told.iter().any(|v| v.has_pending()) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        tokio::time::sleep(Duration::from_millis(50)).await; // the last write
    }

    /// The viewers that said hello.
    pub fn connected(&self) -> Vec<Connected> {
        self.shared
            .viewers
            .lock()
            .unwrap()
            .iter()
            .filter_map(|v| v.connected())
            .collect()
    }

    /// The panes whose connected viewers said a verified hello.
    pub fn verified_panes(&self) -> BTreeSet<String> {
        self.connected()
            .into_iter()
            .filter_map(|c| c.pane_id)
            .collect()
    }

    /// Stops the core: every viewer closed, the socket removed, the lock released.
    pub fn stop(self) {
        drop(self);
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        for t in &self.tasks {
            t.abort();
        }
        for v in self.shared.viewers.lock().unwrap().drain(..) {
            v.close();
        }
        let _ = std::fs::remove_file(&self.sock);
    }
}

pub use start::start;
