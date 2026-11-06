//! The swarm's totals, as the headers and `panorama snapshot --counts` show them.

use super::{HerdrStatus, Swarm};
use crate::model::Status;
use serde::Serialize;

/// What `panorama snapshot --json` prints as `counts`, and the docked header's `?N !N ✗N`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub roots: usize,
    pub agents: usize,
    /// Agents running or reviewing.
    pub running: usize,
    /// Roots with background work in flight, plus agents waiting on theirs.
    pub waiting: usize,
    /// Open inbox questions, over every root.
    pub questions: usize,
    /// Roots herdr reports blocked.
    pub blocked: usize,
    /// Failed agents.
    pub failed: usize,
}

impl Swarm {
    /// The totals.
    pub fn counts(&self) -> Counts {
        let roots = &self.roots;
        let agents = || roots.iter().flat_map(|o| &o.agents);
        let with = |st: &[Status]| agents().filter(|a| st.contains(&a.status)).count();
        Counts {
            roots: roots.len(),
            agents: agents().count(),
            running: with(&[Status::Running, Status::Reviewing]),
            waiting: roots.iter().filter(|o| !o.bg.is_empty()).count()
                + with(&[Status::Background]),
            questions: roots.iter().map(|o| o.inbox.len()).sum(),
            blocked: roots
                .iter()
                .filter(|o| o.herdr_status == HerdrStatus::Blocked)
                .count(),
            failed: with(&[Status::Failed]),
        }
    }
}
