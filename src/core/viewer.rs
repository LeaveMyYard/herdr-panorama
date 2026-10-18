//! One connected viewer (Python `Viewer`): its outbox, what its hello said, whether it is
//! closed. The connection's tasks ([`super::conn`]) read and write; everything else only
//! queues and looks.

use super::Connected;
use super::outbox::Outbox;
use std::sync::{Arc, Mutex};
use tokio::sync::{Notify, watch};

/// What a viewer said it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// A per-tab panel: it gets every state.
    Panel,
    /// The open key: its one request and the answer, no state.
    Key,
}

#[derive(Debug)]
struct Said {
    hello: bool,
    role: Role,
    pane_id: Option<String>,
    tab_id: Option<String>,
    /// Shutdown is queued: nothing after it may push it out of the outbox.
    told_to_go: bool,
}

#[derive(Debug)]
pub struct Viewer {
    /// The connecting process (`SO_PEERCRED`), for the pane check.
    pub pid: Option<i32>,
    said: Mutex<Said>,
    outbox: Mutex<Outbox>,
    /// Something was queued.
    pub(super) wake: Notify,
    closed: watch::Sender<bool>,
}

impl Viewer {
    pub fn new(pid: Option<i32>, outbox: usize) -> Self {
        Self {
            pid,
            said: Mutex::new(Said {
                hello: false,
                role: Role::Panel,
                pane_id: None,
                tab_id: None,
                told_to_go: false,
            }),
            outbox: Mutex::new(Outbox::new(outbox)),
            wake: Notify::new(),
            closed: watch::channel(false).0,
        }
    }

    /// Queues `line` without blocking (a full outbox loses its oldest).
    pub fn send(&self, line: Arc<Vec<u8>>) {
        self.outbox.lock().unwrap().push(line);
        self.wake.notify_one();
    }

    /// The next line to write, if any.
    pub fn next(&self) -> Option<Arc<Vec<u8>>> {
        self.outbox.lock().unwrap().pop()
    }

    pub fn has_pending(&self) -> bool {
        !self.is_closed() && !self.outbox.lock().unwrap().is_empty()
    }

    /// The hello: what it claims, after the pane check.
    pub fn said_hello(&self, role: Role, pane_id: Option<String>, tab_id: Option<String>) {
        let mut s = self.said.lock().unwrap();
        (s.hello, s.role, s.pane_id, s.tab_id) = (true, role, pane_id, tab_id);
    }

    pub fn hello(&self) -> bool {
        self.said.lock().unwrap().hello
    }

    pub fn role(&self) -> Role {
        self.said.lock().unwrap().role
    }

    /// Its verified pane: only then does its `focusing` count.
    pub fn verified_pane(&self) -> Option<String> {
        self.said.lock().unwrap().pane_id.clone()
    }

    /// A panel that said hello and is not being told to go: it gets the states.
    pub fn wants_state(&self) -> bool {
        let s = self.said.lock().unwrap();
        s.hello && s.role == Role::Panel && !s.told_to_go && !self.is_closed()
    }

    /// A panel that said hello (shutdown goes to these).
    pub fn is_panel(&self) -> bool {
        let s = self.said.lock().unwrap();
        s.hello && s.role == Role::Panel && !self.is_closed()
    }

    /// Queues shutdown as the last thing it gets.
    pub fn tell_to_go(&self, line: Arc<Vec<u8>>) {
        self.said.lock().unwrap().told_to_go = true;
        self.send(line);
    }

    pub fn connected(&self) -> Option<Connected> {
        let s = self.said.lock().unwrap();
        (s.hello && !self.is_closed()).then(|| Connected {
            pane_id: s.pane_id.clone(),
            tab_id: s.tab_id.clone(),
            role: s.role,
        })
    }

    pub fn close(&self) {
        self.closed.send_replace(true);
    }

    pub fn is_closed(&self) -> bool {
        *self.closed.borrow()
    }

    /// Resolves once the viewer is closed (at once if it already is).
    pub async fn closed(&self) {
        let mut rx = self.closed.subscribe();
        let _ = rx.wait_for(|c| *c).await;
    }
}
