//! The collector over another machine's mirrored transcripts (Python's
//! `Collector(root=mirror_root(machine), procs=False)` inside `machines.Watcher`): no process
//! checks, no inbox (it is local), the done window and stale time as the config says now.

use super::Collector;
use crate::machines::Collect;
use herdr_agent_kit::herdr_api::Snapshot;
use panorama_core::model::Swarm;

/// The done window and the stale time, in seconds, read when a collect runs.
pub type Windows = dyn FnMut() -> (f64, f64) + Send;

/// A collector of one machine's mirror.
pub struct MirrorCollector {
    /// The collector, made with `procs: false` over the mirror.
    pub collector: Collector,
    /// The windows now.
    pub windows: Box<Windows>,
    /// The time now (POSIX seconds).
    pub clock: Box<dyn FnMut() -> f64 + Send>,
}

impl Collect for MirrorCollector {
    fn continuations(&mut self, named: &[String]) -> Vec<String> {
        self.collector.continuations(named)
    }

    fn collect(&mut self, snapshot: &Snapshot) -> Swarm {
        let (window, stale) = (self.windows)();
        let now = (self.clock)();
        // herdr's snapshot as Python reads it: the JSON object it sent (the typed snapshot
        // keeps every field it does not name in `extra`)
        let snap = serde_json::to_value(snapshot).ok();
        self.collector
            .collect(snap.as_ref(), None, now, window, stale)
    }
}
