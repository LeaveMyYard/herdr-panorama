//! The view filter (all, running, needs me) and which orchestrators it shows (Python
//! `render.FILTERS`, `visible_orchs`, `orch_needs`).

use super::orch_state::{Ages, orch_state};
use super::state::View;
use crate::model::{HerdrStatus, Orchestrator, Status, Swarm};
use serde::{Deserialize, Serialize};

/// What a view shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Filter {
    /// Everything.
    #[default]
    All,
    /// Work in flight (waiting counts, stale does not).
    Running,
    /// What needs the operator.
    Needs,
}

impl Filter {
    /// In the order `f` cycles through them.
    pub const ALL: [Filter; 3] = [Filter::All, Filter::Running, Filter::Needs];

    /// The protocol's name (`state.filter`, the `filter` message).
    pub fn name(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Running => "running",
            Self::Needs => "needs",
        }
    }

    /// The filter of a protocol name; `None` for an unknown one (it is ignored).
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.name() == name)
    }

    /// The header's words for it.
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Running => "running",
            Self::Needs => "needs me",
        }
    }
}

/// The orchestrator needs the operator: herdr reports it blocked, it has open questions, or
/// an agent failed.
pub fn orch_needs(o: &Orchestrator) -> bool {
    o.herdr_status == HerdrStatus::Blocked
        || !o.inbox.is_empty()
        || o.agents().any(|a| a.status == Status::Failed)
}

/// The orchestrators the view's filter shows, in order.
pub fn visible_orchs<'a>(swarm: &'a Swarm, view: &View) -> Vec<&'a Orchestrator> {
    let running = |o: &Orchestrator| {
        orch_state(o, 0.0, Some(Ages::of(view, swarm))).st != Status::Stale
            && (o.threads.iter().any(|t| !t.is_done())
                || o.herdr_status == HerdrStatus::Working
                || !o.bg.is_empty()
                || o.agents().any(|a| a.status == Status::Background))
    };
    swarm
        .orchestrators
        .iter()
        .filter(|o| match view.filter {
            Filter::All => true,
            Filter::Running => running(o),
            Filter::Needs => orch_needs(o),
        })
        .collect()
}

#[cfg(test)]
mod tests;
