//! The app's collects (Python `SwarmApp.refresh_model`), on a thread of their own: one at a
//! time (a refresh that finds one running is skipped), its swarm cleaned for drawing, and a
//! panic in it reported as a failed refresh: the last good model stays.
//!
//! The source is made on that thread (the collector need not be `Send`).

use crate::proto::state::cleaned_swarm;
use panorama_core::model::Swarm;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc as std_mpsc;
use std::thread;
use tokio::sync::mpsc;

/// What makes the swarm: herdr, the transcripts, the inbox (a test passes a fixed one).
pub type Source = Box<dyn FnMut() -> Swarm>;

/// The collect thread's name; the app's panic hook keeps a panic on it off the terminal.
pub const THREAD: &str = "panorama-collect";

/// A collect's outcome.
#[derive(Debug)]
pub enum Collected {
    Swarm(Swarm),
    Failed(String),
}

/// The collect thread, as the actor drives it.
pub struct Worker {
    ask: std_mpsc::Sender<()>,
    busy: bool,
}

impl Worker {
    /// Starts the thread; `make` builds the source on it, and each outcome goes to `out`.
    pub fn spawn(
        make: impl FnOnce() -> Source + Send + 'static,
        out: mpsc::UnboundedSender<Collected>,
    ) -> std::io::Result<Self> {
        let (ask, asked) = std_mpsc::channel::<()>();
        thread::Builder::new()
            .name(THREAD.into())
            .spawn(move || serve(make, &asked, &out))?;
        Ok(Self { ask, busy: false })
    }

    /// Asks for a collect unless one is running. Whether one was asked for.
    pub fn request(&mut self) -> bool {
        if self.busy {
            return false;
        }
        self.busy = self.ask.send(()).is_ok();
        self.busy
    }

    /// The running collect reported (its outcome arrived).
    pub fn done(&mut self) {
        self.busy = false;
    }
}

fn serve(
    make: impl FnOnce() -> Source,
    asked: &std_mpsc::Receiver<()>,
    out: &mpsc::UnboundedSender<Collected>,
) {
    let mut source = match catch_unwind(AssertUnwindSafe(make)) {
        Ok(s) => Some(s),
        Err(p) => {
            let _ = out.send(Collected::Failed(format!("collector: {}", message(&*p))));
            None
        }
    };
    while asked.recv().is_ok() {
        let outcome = match source.as_mut() {
            None => Collected::Failed("collector: not started".into()),
            Some(s) => match catch_unwind(AssertUnwindSafe(s)) {
                Ok(swarm) => Collected::Swarm(cleaned_swarm(swarm)),
                Err(p) => Collected::Failed(format!("panic: {}", message(&*p))),
            },
        };
        if out.send(outcome).is_err() {
            return; // the app ended
        }
    }
}

/// A panic's message.
fn message(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic".into())
}

#[cfg(test)]
mod tests;
