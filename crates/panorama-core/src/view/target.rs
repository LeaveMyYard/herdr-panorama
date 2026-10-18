//! What a click (or the mouse) can hit on a rendered screen (Python's `(kind, identity)`
//! click targets).
//!
//! A click acts on the identity drawn under it at the last paint, never on a row index (the
//! same rule as the cursor).

use super::filter::Filter;

/// A thing on screen that a click acts on.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    /// An orchestrator's whole card or entry (`orch`), by its pane id.
    Orch(String),
    /// The `+ New agent` row (`new`).
    NewAgent,
    /// An orchestrator's `?N`: answer its questions (`answer-orch`).
    AnswerOrch(String),
    /// A filter in the graph header (`filter`).
    Filter(Filter),
    /// An orchestrator's "n complete" card (`done`), by its pane id.
    Done(String),
    /// A thread's card (`thread`).
    Thread {
        pane: String,
        thread: String,
    },
    /// An agent's card or pod (`agent`).
    Agent(AgentRef),
    /// A "n finished" card in the wide graph (`fin`): the thread's own group (`group`
    /// `None`) or the group of the agent `group` spawned.
    Fin {
        pane: String,
        thread: String,
        group: Option<String>,
    },
    /// An agent's `g open pane` (`goto`), `t transcript` (`transcript`), `a answer` (`answer`).
    Goto(AgentRef),
    Transcript(AgentRef),
    Answer(AgentRef),
}

/// An agent where the graph shows it: its orchestrator's pane, its thread, its id.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AgentRef {
    pub pane: String,
    pub thread: String,
    pub agent: String,
}

impl Target {
    /// A whole card (Python `CARDS`): the hover highlight follows these.
    pub fn is_card(&self) -> bool {
        matches!(
            self,
            Self::Orch(_)
                | Self::NewAgent
                | Self::Done(_)
                | Self::Thread { .. }
                | Self::Agent(_)
                | Self::Fin { .. }
        )
    }
}

/// One row span of a target, `x0..x1` at `y`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub x0: i32,
    pub x1: i32,
    pub y: i32,
    pub target: Target,
}
