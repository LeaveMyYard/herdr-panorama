//! The focus events herdr's event stream has shown, stamped on arrival (Python
//! `cockpit.FocusLog`): the start batch reads it to see whether the operator went somewhere
//! while it swapped panels in, and the restore check whether the operator ever focused a
//! restored panel. The app records into it; `live` says the stream is connected.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Notify;

/// A focus event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Focus {
    Tab(String),
    Pane(String),
}

#[derive(Debug, Default)]
pub struct FocusLog {
    events: Mutex<VecDeque<(Instant, Focus)>>,
    added: Notify,
    live: AtomicBool,
}

/// Python's `deque(maxlen=64)`: "ever focused" means within the last 64 events, so a big
/// start batch (2 events per tab) can push out an older focus on a restored panel (parity).
const KEEP: usize = 64;

impl FocusLog {
    pub fn record(&self, ev: Focus) {
        let mut e = self.events.lock().unwrap();
        if e.len() >= KEEP {
            e.pop_front();
        }
        e.push_back((Instant::now(), ev));
        self.added.notify_waiters();
    }

    pub fn set_live(&self, on: bool) {
        self.live.store(on, Ordering::SeqCst);
    }

    pub fn live(&self) -> bool {
        self.live.load(Ordering::SeqCst)
    }

    /// The events since `t0`, oldest first.
    pub fn since(&self, t0: Instant) -> Vec<Focus> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|(t, _)| *t >= t0)
            .map(|(_, e)| e.clone())
            .collect()
    }

    /// Every event kept.
    pub fn all(&self) -> Vec<Focus> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .map(|(_, e)| e.clone())
            .collect()
    }

    /// Until an event since `t0` matches (true), or `timeout` (false).
    pub async fn wait_for(
        &self,
        t0: Instant,
        mut pred: impl FnMut(&Focus) -> bool,
        timeout: Duration,
    ) -> bool {
        let end = tokio::time::Instant::now() + timeout;
        loop {
            let added = self.added.notified();
            if self.since(t0).iter().any(&mut pred) {
                return true;
            }
            if tokio::time::timeout_at(end, added).await.is_err() {
                return self.since(t0).iter().any(&mut pred);
            }
        }
    }
}
