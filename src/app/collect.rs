//! The app's refreshes (Python `SwarmApp.refresh_model`), on a thread of their own: one at a
//! time (a refresh that finds one running is skipped), its swarm cleaned for drawing, and a
//! failure reported as a failed refresh: the last good model stays.
//!
//! The refresher is the core's [`Producer`] (this machine's swarm, herdr's saved machines,
//! the unseen marks), made on that thread. Quitting stops it without waiting it out
//! ([`Worker::stop`]).

use crate::produce::{Producer, Refreshed};
use crate::proto::state::cleaned_swarm;
use crate::unseen::Event;
use panorama_core::model::Swarm;
use panorama_core::view::{Marks, Unseen};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc as std_mpsc;
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;

/// What refreshes the app (the [`Producer`]; a test passes its own).
pub trait Refresher {
    /// One refresh; blocks.
    fn refresh(&mut self) -> Refreshed;
    /// Stop what it runs in the background (the remote machines' watchers).
    fn stop(&mut self);
}

impl Refresher for Producer {
    fn refresh(&mut self) -> Refreshed {
        Producer::refresh(self)
    }

    fn stop(&mut self) {
        Producer::stop(self);
    }
}

/// The refresh thread's name; the app's panic hook keeps a panic on it off the terminal.
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
        thread::Builder::new()
            .name(THREAD.into())
            .spawn(move || serve(make, &asked, &out, &done))?;
        Ok(Self {
            ask,
            stopped,
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

    /// Quitting: the refresher stops (a remote sync under way is killed at once), waited for
    /// at most `within`; the thread is then left to finish on its own (a forwarded snapshot
    /// under way can take herdr's CLI up to 8 s, and Python quits at once). Whether it had
    /// stopped by then.
    pub fn stop(&self, within: Duration) -> bool {
        let _ = self.ask.send(Ask::Stop);
        self.stopped.recv_timeout(within).is_ok()
    }
}

fn serve<R: Refresher>(
    make: impl FnOnce() -> R,
    asked: &std_mpsc::Receiver<Ask>,
    out: &mpsc::UnboundedSender<Collected>,
    done: &std_mpsc::Sender<()>,
) {
    let mut refresher = match catch_unwind(AssertUnwindSafe(make)) {
        Ok(r) => Some(r),
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
