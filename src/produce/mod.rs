//! The core's producer (Python `SwarmApp.refresh_model` and its `source`): one refresh is this
//! machine's swarm (one collector kept across refreshes, [`crate::snapshot::source`]), herdr's
//! saved machines merged in ([`crate::machines::Remotes`], started on the first refresh as in
//! Python), and the unseen marks ([`crate::unseen::Tracker`], file IO and its lock included).
//! What is published around it (focus, tabs, panels, filter) is the core's and the app's.
//!
//! [`Producer::refresh`] blocks: run it on a worker thread, one at a time (Python's single
//! flight is its caller's). A refresh that panics keeps the last result with the error, so a
//! bad transcript never kills the worker. Dropping the producer stops the remote watchers.

pub mod remote;

use crate::machines::Remotes;
use crate::unseen::{self, Marks, Tracker};
use herdr_agent_kit::internal::plugin_paths::PANORAMA_ID;
use herdr_agent_kit::plugin_env::{ConfigFile, PluginEnv};
use panorama_core::model::Swarm;
use serde_json::{Map, Value};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// One refresh's result: what the core publishes and the app shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Refreshed {
    /// This machine's swarm with every watched machine's merged in.
    pub swarm: Swarm,
    /// The unseen marks for it.
    pub marks: Marks,
    /// How long the refresh took.
    pub took: Duration,
    /// Why the latest refresh failed, when it did (the swarm and marks are the last good ones).
    pub error: Option<String>,
}

/// Makes the remote machines' watchers (started on the first refresh).
pub type StartRemotes = Box<dyn FnOnce() -> Remotes + Send>;

/// The producer: the local source, the remote machines, the unseen tracker, the last result.
pub struct Producer {
    source: Box<dyn FnMut() -> Swarm + Send>,
    start_remotes: Option<StartRemotes>,
    remotes: Option<Remotes>,
    tracker: Arc<Tracker>,
    clock: Box<dyn Fn() -> f64 + Send>,
    last: Refreshed,
}

fn panic_text(p: &(dyn std::any::Any + Send)) -> String {
    let what = p
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_default();
    format!("panic: {what}")
}

impl Producer {
    /// A producer from its parts (tests; the core and the app use [`Producer::from_env`]).
    pub fn new(
        source: Box<dyn FnMut() -> Swarm + Send>,
        start_remotes: Option<StartRemotes>,
        tracker: Arc<Tracker>,
        clock: Box<dyn Fn() -> f64 + Send>,
    ) -> Self {
        Self {
            source,
            start_remotes,
            remotes: None,
            tracker,
            clock,
            last: Refreshed::default(),
        }
    }

    /// The real producer: herdr and the transcripts (`self_pane`: the view's own pane), the
    /// saved machines, and the unseen state file of this herdr server (`highlight_unseen`).
    pub fn from_env(env: PluginEnv, self_pane: Option<String>) -> Self {
        let herdr = herdr_agent_kit::herdr_api::blocking::Herdr::from_env().with_client("panorama");
        let config = Mutex::new(ConfigFile::resolve(&env, || {
            herdr.plugin_config_dir(PANORAMA_ID).ok().flatten()
        }));
        let enabled = move || {
            let cfg: Map<String, Value> = config
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .load_as()
                .unwrap_or_default();
            unseen::enabled(&cfg)
        };
        let tracker = Arc::new(Tracker::new(unseen::default_path(&env), Box::new(enabled)));
        let remote_env = env.clone();
        Self::new(
            Box::new(crate::snapshot::source(env, self_pane)),
            Some(Box::new(move || remote::start(&remote_env, herdr))),
            tracker,
            Box::new(crate::snapshot::now),
        )
    }

    /// The tracker, for the UI's `seen` and `save_seen` (off the UI thread).
    pub fn tracker(&self) -> Arc<Tracker> {
        self.tracker.clone()
    }

    /// One refresh: the local swarm, the machines merged, the unseen marks. A refresh that
    /// panics keeps the last result, with the error.
    pub fn refresh(&mut self) -> Refreshed {
        let started = Instant::now();
        let out = catch_unwind(AssertUnwindSafe(|| {
            let mut swarm = (self.source)();
            if let Some(start) = self.start_remotes.take() {
                self.remotes = Some(start());
            }
            if let Some(r) = self.remotes.as_mut() {
                r.merge(&mut swarm);
            }
            let marks = self.tracker.update(&swarm, (self.clock)());
            (swarm, marks)
        }));
        match out {
            Ok((swarm, marks)) => {
                self.last = Refreshed {
                    swarm,
                    marks,
                    took: started.elapsed(),
                    error: None,
                }
            }
            Err(p) => {
                self.last.error = Some(panic_text(&*p));
                self.last.took = started.elapsed();
            }
        }
        self.last.clone()
    }

    /// Stop the remote machines' watchers and wait for their threads: a refresh under way
    /// ends first (its snapshot and rsync are bounded by their timeouts), so no watcher thread
    /// or rsync child outlives the producer.
    pub fn stop(&mut self) {
        if let Some(mut r) = self.remotes.take() {
            r.stop();
            for w in r.watchers.drain(..) {
                w.join();
            }
        }
    }
}

impl Drop for Producer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests;
