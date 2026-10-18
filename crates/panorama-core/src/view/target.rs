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
}

impl Target {
    /// A whole card (Python `CARDS`): the hover highlight follows these.
    pub fn is_card(&self) -> bool {
        matches!(self, Self::Orch(_) | Self::NewAgent)
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
