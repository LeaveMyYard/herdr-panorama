//! What a click (or the mouse) can hit on a rendered screen (Python's `(kind, identity)`
//! click targets).
//!
//! A click acts on the identity drawn under it at the last paint, never on a row index (the
//! same rule as the cursor).

use super::filter::Filter;

/// A thing on screen that a click acts on.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    /// A root's whole card or entry (`orch`), by its pane id.
    Orch(String),
    /// The `+ New agent` row (`new`).
    NewAgent,
    /// A root's `?N`: answer its questions (`answer-orch`).
    AnswerOrch(String),
    /// A filter in the graph header (`filter`).
    Filter(Filter),
    /// A root's `g open pane` and `t transcript` hints, by its pane id.
    GotoOrch(String),
    TranscriptOrch(String),
    /// An agent's card or pod (`agent`).
    Agent(AgentRef),
    /// A "n finished" card in the wide graph (`fin`): the root's own group (`group` `None`)
    /// or the group of the agent `group` spawned.
    Fin {
        pane: String,
        group: Option<String>,
    },
    /// An agent's `g open pane` (`goto`), `t transcript` (`transcript`), `a answer` (`answer`).
    Goto(AgentRef),
    Transcript(AgentRef),
    Answer(AgentRef),
}

/// An agent where the graph shows it: its root's pane and its id (unique in the root).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AgentRef {
    pub pane: String,
    pub agent: String,
}

impl Target {
    /// A whole card (Python `CARDS`): the hover highlight follows these.
    pub fn is_card(&self) -> bool {
        matches!(
            self,
            Self::Orch(_) | Self::NewAgent | Self::Agent(_) | Self::Fin { .. }
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
