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

/// Makes the remote machines' watchers: called on the first refresh, and again on the next one
/// if it failed (Python leaves `remotes` unset and tries again).
pub type StartRemotes = Box<dyn FnMut() -> Remotes + Send>;

/// The longest error a failed refresh keeps, in characters (Python's `message[:200]`).
pub const ERROR_CHARS: usize = 200;

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

/// `highlight_unseen` from panorama's config, read when a refresh asks (the file is re-read only
/// when it changed).
pub fn unseen_enabled(config: ConfigFile) -> Box<dyn Fn() -> bool + Send + Sync> {
    let config = Mutex::new(config);
    Box::new(move || {
        let cfg: Map<String, Value> = config
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .load_as()
            .unwrap_or_default();
        unseen::enabled(&cfg)
    })
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
        let config =
            ConfigFile::resolve(&env, || herdr.plugin_config_dir(PANORAMA_ID).ok().flatten());
        let tracker = Arc::new(Tracker::new(
            unseen::default_path(&env),
            unseen_enabled(config),
        ));
        let remote_env = env.clone();
        Self::new(
            Box::new(crate::snapshot::source(env, self_pane)),
            Some(Box::new(move || remote::start(&remote_env, herdr.clone()))),
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
            if self.remotes.is_none()
                && let Some(start) = self.start_remotes.as_mut()
            {
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
                self.last.error = Some(crate::collect::py::chars(&panic_text(&*p), ERROR_CHARS));
                self.last.took = started.elapsed();
            }
        }
        self.last.clone()
    }

    /// Stop the remote machines' watchers and wait for their threads. A sync under way is
    /// killed at once (its rsync's own process group), the rest of the refresh is skipped; only
    /// a forwarded snapshot under way is waited for (herdr's CLI, at most 8 s). So no watcher
    /// thread or rsync child outlives the producer, and stopping does not wait out a hung ssh.
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
