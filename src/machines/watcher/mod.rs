//! A machine's background refresh (Python `machines.Watcher`): its own thread, every
//! [`REMOTE_SECONDS`], never blocking the view, which reads the latest [`Outcome`]. A refresh
//! that panics is that machine's error; the view and the other machines go on.

pub mod refresh;

pub use refresh::{Collect, Outcome, Refresher, SnapshotApi, error_text, session_ids};

use super::{Machine, REMOTE_SECONDS};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

/// The stop flag a watcher's thread waits on between refreshes.
#[derive(Debug, Default)]
struct Stop {
    stopped: Mutex<bool>,
    wake: Condvar,
}

impl Stop {
    fn set(&self) {
        *self.stopped.lock().unwrap_or_else(PoisonError::into_inner) = true;
        self.wake.notify_all();
    }

    fn is_set(&self) -> bool {
        *self.stopped.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Waits up to `timeout`; whether the flag is set.
    fn wait(&self, timeout: Duration) -> bool {
        let held = self.stopped.lock().unwrap_or_else(PoisonError::into_inner);
        let (held, _) = self
            .wake
            .wait_timeout_while(held, timeout, |stopped| !*stopped)
            .unwrap_or_else(PoisonError::into_inner);
        *held
    }
}

/// Whether a watcher was stopped (see [`Watcher::stop_flag`]).
#[derive(Clone, Debug)]
pub struct StopFlag(Arc<Stop>);

impl StopFlag {
    /// Whether the watcher was stopped.
    pub fn is_set(&self) -> bool {
        self.0.is_set()
    }
}

/// One machine's watcher: its latest outcome, and its thread when started.
#[derive(Debug)]
pub struct Watcher {
    /// The machine.
    pub machine: Machine,
    outcome: Arc<Mutex<Outcome>>,
    stop: Arc<Stop>,
    thread: Option<JoinHandle<()>>,
}

fn panic_text(p: &(dyn std::any::Any + Send)) -> String {
    let what = p
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_default();
    format!("panic: {what}")
}

impl Watcher {
    /// A watcher with no thread: nothing tried yet.
    pub fn idle(machine: Machine) -> Self {
        Self {
            machine,
            outcome: Arc::default(),
            stop: Arc::default(),
            thread: None,
        }
    }

    /// A watcher refreshing on its own thread from now on.
    pub fn spawn(mut refresher: Refresher) -> Self {
        let mut w = Self::idle(refresher.machine.clone());
        let (outcome, stop) = (w.outcome.clone(), w.stop.clone());
        let name = format!("machine-{}", w.machine.label);
        let run = move || {
            while !stop.is_set() {
                let last = outcome
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone();
                let next = catch_unwind(AssertUnwindSafe(|| refresher.refresh(&last)))
                    .unwrap_or_else(|p| last.failed(panic_text(&*p)));
                *outcome.lock().unwrap_or_else(PoisonError::into_inner) = next;
                if stop.wait(REMOTE_SECONDS) {
                    break;
                }
            }
        };
        w.thread = std::thread::Builder::new().name(name).spawn(run).ok();
        w
    }

    /// The latest outcome.
    pub fn outcome(&self) -> Outcome {
        self.outcome
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Replace the latest outcome (tests, and a view that restores one).
    pub fn set_outcome(&self, outcome: Outcome) {
        *self.outcome.lock().unwrap_or_else(PoisonError::into_inner) = outcome;
    }

    /// Stop after the refresh under way (a sync under way finishes under the machine's lock).
    pub fn stop(&self) {
        self.stop.set();
    }

    /// Whether [`Watcher::stop`] was called.
    pub fn is_stopped(&self) -> bool {
        self.stop.is_set()
    }

    /// A handle that tells whether this watcher was stopped, after it is gone too.
    pub fn stop_flag(&self) -> StopFlag {
        StopFlag(self.stop.clone())
    }

    /// Whether its thread runs.
    pub fn is_running(&self) -> bool {
        self.thread.as_ref().is_some_and(|t| !t.is_finished())
    }

    /// Stop it and wait for its thread to end.
    pub fn join(mut self) {
        self.stop();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests;
