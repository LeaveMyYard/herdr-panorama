//! One refresh of one machine (Python `Watcher.refresh`): its herdr snapshot, its sessions'
//! transcripts mirrored (following continuations, #39), the collector over the mirror, and the
//! result namespaced. A failed snapshot keeps the last good swarm, with the error.

use crate::machines::mirror::STOPPED;
use crate::machines::sync::Syncer;
use crate::machines::{CONTINUE_ROUNDS, Machine, cut, namespace};
use herdr_agent_kit::herdr_api::{HerdrError, Snapshot, blocking};
use panorama_core::model::Swarm;
use std::sync::Arc;
use std::time::Instant;

/// A machine's herdr snapshot, through herdr's forwarding.
pub trait SnapshotApi: Send + Sync {
    /// `herdr --machine <id> api snapshot`, or why not.
    fn machine_snapshot(&self, machine_id: &str) -> Result<Snapshot, String>;
}

/// The error's text as the view shows it: herdr's own words where it gave them.
pub fn error_text(e: &HerdrError) -> String {
    match e {
        HerdrError::Garbled(why) => why.clone(),
        other => other.to_string(),
    }
}

impl SnapshotApi for blocking::Herdr {
    fn machine_snapshot(&self, machine_id: &str) -> Result<Snapshot, String> {
        blocking::Herdr::machine_snapshot(self, machine_id).map_err(|e| error_text(&e))
    }
}

/// The collector, pointed at a machine's mirror (transcripts only, no process checks).
pub trait Collect: Send {
    /// The sessions `named` continued in, as far as the transcripts read so far tell
    /// (including a next one not mirrored yet), none of them in `named`.
    fn continuations(&mut self, named: &[String]) -> Vec<String>;
    /// The swarm of this snapshot (no inbox: the inbox is local).
    fn collect(&mut self, snapshot: &Snapshot) -> Swarm;
}

/// A machine's latest result (Python `machines.Result`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    /// The last good swarm, namespaced.
    pub swarm: Option<Swarm>,
    /// When it was made.
    pub at: Option<Instant>,
    /// Why the latest refresh failed, if it did.
    pub error: Option<String>,
    /// Whether a refresh has run.
    pub tried: bool,
}

impl Outcome {
    /// The last good result kept, with `error`.
    pub fn failed(&self, error: String) -> Self {
        Self {
            swarm: self.swarm.clone(),
            at: self.at,
            error: Some(error),
            tried: true,
        }
    }
}

/// The session ids a snapshot names (an agent session of kind `id`, or of no kind).
pub fn session_ids(snapshot: &Snapshot) -> Vec<String> {
    snapshot
        .agents
        .iter()
        .filter_map(|a| a.agent_session.as_ref()?.id().map(str::to_owned))
        .collect()
}

/// What one machine's refreshes need.
pub struct Refresher {
    /// The machine.
    pub machine: Machine,
    /// Its snapshot.
    pub api: Arc<dyn SnapshotApi>,
    /// Its mirror.
    pub syncer: Arc<dyn Syncer>,
    /// The collector over its mirror.
    pub collector: Box<dyn Collect>,
}

impl Refresher {
    /// One refresh after `last`; `cancelled()` says the watcher is stopping (checked between
    /// steps, and by the syncs while rsync runs: a stop never waits out a sync).
    pub fn refresh(&mut self, last: &Outcome, cancelled: &dyn Fn() -> bool) -> Outcome {
        let snap = match self.api.machine_snapshot(&self.machine.id) {
            Ok(snap) => snap,
            Err(e) => {
                return last.failed(if e.is_empty() {
                    "unreachable".into()
                } else {
                    e
                });
            }
        };
        if cancelled() {
            return last.failed(STOPPED.into());
        }
        let named = session_ids(&snap);
        let mut sessions = named.clone();
        sessions.extend(self.collector.continuations(&named));
        let mut err = self.syncer.sync(&self.machine, &sessions, cancelled);
        let mut swarm = self.collector.collect(&snap);
        for _ in 0..CONTINUE_ROUNDS {
            // herdr names a conversation's first session; its mirrored transcript may say it
            // continued in another one: mirror that too, now, and look again
            let more: Vec<String> = self
                .collector
                .continuations(&named)
                .into_iter()
                .filter(|s| !sessions.contains(s))
                .collect();
            if err.is_some() || more.is_empty() || cancelled() {
                break;
            }
            sessions.extend(more);
            err = self.syncer.sync(&self.machine, &sessions, cancelled);
            swarm = self.collector.collect(&snap);
        }
        let mut swarm = namespace(swarm, &self.machine);
        if let Some(e) = err {
            let label = &self.machine.label;
            swarm
                .errors
                .push(cut(format!("{label}: transcripts not synced: {e}")));
        }
        Outcome {
            swarm: Some(swarm),
            at: Some(Instant::now()),
            error: None,
            tried: true,
        }
    }
}

#[cfg(test)]
pub(super) mod tests;
