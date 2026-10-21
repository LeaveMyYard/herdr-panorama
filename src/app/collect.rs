//! The app's refreshes (Python `SwarmApp.refresh_model`), on a thread of their own: one at a
//! time (a refresh that finds one running is skipped), its swarm cleaned for drawing, and a
//! failure reported as a failed refresh: the last good model stays.
//!
//! The refresher is the core's [`Producer`] (this machine's swarm, herdr's saved machines,
//! the unseen marks), made on that thread. Quitting stops its remote watchers at once, even
//! mid-refresh, and does not wait the refresh out ([`Worker::stop`]). The UI marks agents
//! seen through its tracker ([`Worker::tracker`]).

use crate::produce::{Producer, Refreshed};
use crate::proto::state::cleaned_swarm;
use crate::unseen::{Event, Tracker};
use panorama_core::model::Swarm;
use panorama_core::view::{Marks, Unseen};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;

/// What refreshes the app (the [`Producer`]; a test passes its own).
pub trait Refresher {
    /// One refresh; blocks.
    fn refresh(&mut self) -> Refreshed;
    /// Stop what it runs in the background (the remote machines' watchers).
    fn stop(&mut self);
    /// What other threads may use while it refreshes.
    fn handles(&self) -> Handles;
}

/// A refresher's handles for other threads.
pub struct Handles {
    /// Stops the background work now, even mid-refresh, without waiting for it.
    pub halt: Box<dyn Fn() + Send + Sync>,
    /// The unseen tracker, for the UI's `seen` and `save_seen`.
    pub tracker: Option<Arc<Tracker>>,
}

impl Refresher for Producer {
    fn refresh(&mut self) -> Refreshed {
        Producer::refresh(self)
    }

    fn stop(&mut self) {
        Producer::stop(self);
    }

    fn handles(&self) -> Handles {
        let halt = self.halt();
        Handles {
            halt: Box::new(move || halt.stop()),
            tracker: Some(self.tracker()),
        }
    }
}

/// The name of the app's background threads (the refreshes, [`crate::app::seen`]'s saves);
/// the app's panic hook keeps a panic on them off the terminal.
pub const THREAD: &str = "panorama-collect";

/// A refresh's outcome.
#[derive(Debug)]
pub enum Collected {
    /// The swarm (cleaned for drawing) and its unseen marks, as the view reads them.
    Swarm(Swarm, Marks),
    Failed(String),
}

/// What the actor asks the thread.
enum Ask {
    Refresh,
    Stop,
}

/// The refresh thread, as the actor drives it.
pub struct Worker {
    ask: std_mpsc::Sender<Ask>,
    stopped: std_mpsc::Receiver<()>,
    handles: Arc<OnceLock<Handles>>,
    busy: bool,
}

impl Worker {
    /// Starts the thread; `make` builds the refresher on it, and each outcome goes to `out`.
    pub fn spawn<R: Refresher + 'static>(
        make: impl FnOnce() -> R + Send + 'static,
        out: mpsc::UnboundedSender<Collected>,
    ) -> std::io::Result<Self> {
        let (ask, asked) = std_mpsc::channel::<Ask>();
        let (done, stopped) = std_mpsc::channel::<()>();
        let handles: Arc<OnceLock<Handles>> = Arc::default();
        let made = handles.clone();
        thread::Builder::new()
            .name(THREAD.into())
            .spawn(move || serve(make, &made, &asked, &out, &done))?;
        Ok(Self {
            ask,
            stopped,
            handles,
            busy: false,
        })
    }

    /// Asks for a refresh unless one is running. Whether one was asked for.
    pub fn request(&mut self) -> bool {
        if self.busy {
            return false;
        }
        self.busy = self.ask.send(Ask::Refresh).is_ok();
        self.busy
    }

    /// The running refresh reported (its outcome arrived).
    pub fn done(&mut self) {
        self.busy = false;
    }

    /// The refresher's unseen tracker, once it is made.
    pub fn tracker(&self) -> Option<Arc<Tracker>> {
        self.handles.get().and_then(|h| h.tracker.clone())
    }

    /// Quitting: the refresher's background work is halted from here at once (a remote sync
    /// under way is killed, even while a refresh runs), then the thread stops it after its
    /// refresh, waited for at most `within`. The thread is then left to finish on its own:
    /// a refresh's herdr calls, or a forwarded snapshot's (read-only, up to 8 s), may outlive
    /// the exit; Python quits at once. Whether it had stopped by then.
    pub fn stop(&self, within: Duration) -> bool {
        if let Some(h) = self.handles.get() {
            (h.halt)();
        }
        let _ = self.ask.send(Ask::Stop);
        self.stopped.recv_timeout(within).is_ok()
    }
}

fn serve<R: Refresher>(
    make: impl FnOnce() -> R,
    handles: &OnceLock<Handles>,
    asked: &std_mpsc::Receiver<Ask>,
    out: &mpsc::UnboundedSender<Collected>,
    done: &std_mpsc::Sender<()>,
) {
    let mut refresher = match catch_unwind(AssertUnwindSafe(make)) {
        Ok(r) => {
            let _ = handles.set(r.handles());
            Some(r)
        }
        Err(p) => {
            let _ = out.send(Collected::Failed(format!("collector: {}", message(&*p))));
            None
        }
    };
    while let Ok(Ask::Refresh) = asked.recv() {
        let outcome = match refresher.as_mut() {
            None => Collected::Failed("collector: not started".into()),
            Some(r) => match catch_unwind(AssertUnwindSafe(|| r.refresh())) {
                Ok(Refreshed {
                    error: Some(why), ..
                }) => Collected::Failed(why),
                Ok(Refreshed { swarm, marks, .. }) => Collected::Swarm(
                    cleaned_swarm(swarm),
                    marks.into_iter().map(|(id, ev)| (id, unseen(ev))).collect(),
                ),
                Err(p) => Collected::Failed(format!("panic: {}", message(&*p))),
            },
        };
        if out.send(outcome).is_err() {
            break; // the app ended
        }
    }
    if let Some(r) = refresher.as_mut() {
        r.stop();
    }
    let _ = done.send(());
}

/// A tracker's mark as the view shows it (the core sends the same word, [`Event::code`]).
fn unseen(ev: Event) -> Unseen {
    match ev {
        Event::Fail => Unseen::Fail,
        Event::Wait => Unseen::Wait,
        Event::Done => Unseen::Done,
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
