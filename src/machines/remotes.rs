//! Every machine's watcher, and their results merged into the local swarm (Python
//! `machines.Remotes`). A changed `machines` setting applies without a restart: the watchers
//! of machines still selected keep running, removed ones stop, added ones start.

use super::select::{Selection, Source, saved};
use super::watcher::Watcher;
use super::{Machine, STALE_SECONDS, cut};
use panorama_core::model::Swarm;
use std::time::Instant;

/// Makes a machine's watcher (started, outside tests).
pub type Spawn = dyn FnMut(Machine) -> Watcher + Send;

/// The watchers, the setting they were started for, and where both come from.
pub struct Remotes {
    /// One per watched machine.
    pub watchers: Vec<Watcher>,
    chosen: Selection,
    source: Box<dyn Source>,
    spawn: Box<Spawn>,
}

impl Remotes {
    /// A watcher for each saved machine the setting selects.
    pub fn start(mut source: Box<dyn Source>, mut spawn: Box<Spawn>) -> Self {
        let watchers = saved(&mut *source).into_iter().map(&mut spawn).collect();
        let chosen = source.selection();
        Self {
            watchers,
            chosen,
            source,
            spawn,
        }
    }

    /// Re-read the setting; only when it changed, re-read herdr's machines and replace the
    /// watchers. A machine kept with the same ssh target keeps its watcher, never restarted.
    pub fn follow_config(&mut self) {
        let sel = self.source.selection();
        if sel == self.chosen {
            return;
        }
        self.chosen = sel;
        let mut wanted = saved(&mut *self.source);
        let mut kept = Vec::new();
        for w in self.watchers.drain(..) {
            let same = |m: &Machine| m.id == w.machine.id && m.target == w.machine.target;
            if let Some(i) = wanted.iter().position(same) {
                wanted.remove(i);
                kept.push(w);
            } else {
                w.stop(); // a sync under way finishes under the machine's lock
            }
        }
        kept.extend(wanted.into_iter().map(&mut self.spawn));
        self.watchers = kept;
    }

    /// [`Remotes::merge_at`] now.
    pub fn merge(&mut self, local: &mut Swarm) {
        self.merge_at(local, Instant::now());
    }

    /// Follow the setting, then add every machine's latest swarm and errors to `local`: a
    /// machine that never answered is unreachable; one whose last answer failed or is older
    /// than [`STALE_SECONDS`] is stale.
    pub fn merge_at(&mut self, local: &mut Swarm, now: Instant) {
        self.follow_config();
        for w in &self.watchers {
            let res = w.outcome();
            let label = &w.machine.label;
            let error = res.error.as_deref();
            match &res.swarm {
                Some(swarm) => {
                    local
                        .orchestrators
                        .extend(swarm.orchestrators.iter().cloned());
                    local.errors.extend(swarm.errors.iter().cloned());
                    let old = res
                        .at
                        .is_none_or(|at| now.duration_since(at) > STALE_SECONDS);
                    if error.is_some() || old {
                        let why = error.unwrap_or("no answer");
                        local.errors.push(cut(format!("{label}: stale ({why})")));
                    }
                }
                None if res.tried => {
                    let why = error.unwrap_or("None");
                    local
                        .errors
                        .push(cut(format!("{label}: unreachable ({why})")));
                }
                None => {}
            }
        }
    }

    /// Stop every watcher.
    pub fn stop(&self) {
        for w in &self.watchers {
            w.stop();
        }
    }
}

#[cfg(test)]
mod tests;
