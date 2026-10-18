//! The swarm's totals (Python `Swarm.counts()`), as the headers show them.

use super::{HerdrStatus, Swarm};
use crate::model::Status;
use serde::Serialize;

/// What `panorama snapshot --json` prints as `counts`, and the docked header's `?N !N ✗N`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub orchestrators: usize,
    pub threads: usize,
    /// Threads not done (a failed one counts).
    pub active_threads: usize,
    pub agents: usize,
    /// Agents running or reviewing.
    pub running: usize,
    /// Sessions with background work in flight, plus agents waiting on theirs.
    pub waiting: usize,
    /// Open inbox questions, over every session.
    pub questions: usize,
    /// Sessions herdr reports blocked.
    pub blocked: usize,
    /// Failed agents.
    pub failed: usize,
}

impl Swarm {
    /// The totals (Python `Swarm.counts()`).
    pub fn counts(&self) -> Counts {
        let orchs = &self.orchestrators;
        let agents = || orchs.iter().flat_map(|o| o.agents());
        let with = |st: &[Status]| agents().filter(|a| st.contains(&a.status)).count();
        Counts {
            orchestrators: orchs.len(),
            threads: orchs.iter().map(|o| o.threads.len()).sum(),
            active_threads: orchs
                .iter()
                .flat_map(|o| &o.threads)
                .filter(|t| !t.is_done())
                .count(),
            agents: agents().count(),
            running: with(&[Status::Running, Status::Reviewing]),
            waiting: orchs.iter().filter(|o| !o.bg.is_empty()).count()
                + with(&[Status::Background]),
            questions: orchs.iter().map(|o| o.inbox.len()).sum(),
            blocked: orchs
                .iter()
                .filter(|o| o.herdr_status == HerdrStatus::Blocked)
                .count(),
            failed: with(&[Status::Failed]),
        }
    }
}
