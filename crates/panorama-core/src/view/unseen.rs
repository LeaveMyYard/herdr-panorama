//! The unseen marks (design turn 6): what happened to an agent the operator has not looked
//! at yet, and which mark a group of agents shows.

use crate::model::{Agent, Status};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An unseen event, most urgent first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Unseen {
    /// It failed.
    Fail,
    /// It asks something.
    Wait,
    /// It finished.
    Done,
}

impl Unseen {
    /// The status whose colour the mark pulses in.
    pub fn status(self) -> Status {
        match self {
            Self::Fail => Status::Failed,
            Self::Wait => Status::Waiting,
            Self::Done => Status::Done,
        }
    }
}

/// The unseen marks by agent id (the core's `state.unseen`).
pub type Marks = HashMap<String, Unseen>;

/// The mark a group shows: its most urgent event, and whether it pulses quietly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark {
    pub event: Unseen,
    pub quiet: bool,
}

/// The most urgent unseen event among `agents`; on a card that rolls a group up (`roll`) a
/// finished agent pulses quietly (Python `mark_of`).
pub fn mark_of<'a>(
    agents: impl IntoIterator<Item = &'a Agent>,
    marks: &Marks,
    roll: bool,
) -> Option<Mark> {
    let event = agents
        .into_iter()
        .filter_map(|a| marks.get(&a.id).copied())
        .min()?;
    Some(Mark {
        event,
        quiet: roll && event == Unseen::Done,
    })
}
