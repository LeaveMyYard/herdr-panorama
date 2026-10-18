//! The status vocabulary (Python `model.py`: the codes, `GLYPH`, `LABEL`, `ORDER`, `live`,
//! `urgency`).
//!
//! On the wire a status is its short code (`"run"`, `"wait"`, …). A code this version does
//! not know reads as [`Status::Unknown`] and keeps its text: it is drawn as Python draws an
//! unknown code (glyph `?`, the default colour, least urgent, live).

use serde::{Deserialize, Serialize};

/// The running spinner: the 10-frame braille dots cycle, one frame per 250 ms. Its length is
/// the cycle.
pub const SPIN: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Waiting (design turn 8): one braille dot circling the cell, a step per second.
pub const CLOCK: [char; 8] = ['⠁', '⠈', '⠐', '⠠', '⢀', '⡀', '⠄', '⠂'];

/// An agent's status, or a state the views derive (`Stale` is never an agent status).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(from = "String", into = "String")]
pub enum Status {
    /// `q`: queued, or idle.
    Queued,
    /// `run`: running.
    Running,
    /// `rev`: reviewing.
    Reviewing,
    /// `wait`: needs an answer.
    Waiting,
    /// `blk`: blocked on a permission prompt.
    Blocked,
    /// `fail`: failed.
    Failed,
    /// `done`: done.
    Done,
    /// `cxl`: cancelled.
    Cancelled,
    /// `inf`: the ending was inferred from silence, not recorded.
    Inferred,
    /// `lost`: started, never stopped, and its process is gone.
    Lost,
    /// `bg`: its turn ended, but background work it started is still in flight.
    Background,
    /// `stale`: an orchestrator idle for longer than `stale_after` (views only).
    Stale,
    /// A code this version does not know, as it was sent.
    Unknown(String),
}

/// Most urgent first (Python `ORDER`); `Stale` and unknown codes come after all of these.
const ORDER: [Status; 11] = [
    Status::Failed,
    Status::Blocked,
    Status::Waiting,
    Status::Reviewing,
    Status::Running,
    Status::Background,
    Status::Queued,
    Status::Lost,
    Status::Inferred,
    Status::Done,
    Status::Cancelled,
];

impl Status {
    /// The status for a wire code; an unknown code is kept as [`Status::Unknown`].
    pub fn from_code(code: &str) -> Self {
        match code {
            "q" => Self::Queued,
            "run" => Self::Running,
            "rev" => Self::Reviewing,
            "wait" => Self::Waiting,
            "blk" => Self::Blocked,
            "fail" => Self::Failed,
            "done" => Self::Done,
            "cxl" => Self::Cancelled,
            "inf" => Self::Inferred,
            "lost" => Self::Lost,
            "bg" => Self::Background,
            "stale" => Self::Stale,
            other => Self::Unknown(other.to_owned()),
        }
    }

    /// The wire code.
    pub fn code(&self) -> &str {
        match self {
            Self::Queued => "q",
            Self::Running => "run",
            Self::Reviewing => "rev",
            Self::Waiting => "wait",
            Self::Blocked => "blk",
            Self::Failed => "fail",
            Self::Done => "done",
            Self::Cancelled => "cxl",
            Self::Inferred => "inf",
            Self::Lost => "lost",
            Self::Background => "bg",
            Self::Stale => "stale",
            Self::Unknown(code) => code,
        }
    }

    /// The still glyph (Python `GLYPH`; `?` for an unknown code). Running and waiting
    /// animate in the views; this is their first frame's shape.
    pub fn glyph(&self) -> &'static str {
        match self {
            Self::Queued => "○",
            Self::Running => "⠋",
            Self::Reviewing => "»",
            Self::Waiting => "?",
            Self::Blocked => "!",
            Self::Done => "✓",
            Self::Failed => "✗",
            Self::Cancelled => "-",
            Self::Inferred => "✓?",
            Self::Lost => "~",
            Self::Background => "⠁",
            Self::Stale => "◌",
            Self::Unknown(_) => "?",
        }
    }

    /// The words for it (Python `LABEL`; the code itself when unknown).
    pub fn label(&self) -> &str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Reviewing => "reviewing",
            Self::Waiting => "needs answer",
            Self::Blocked => "blocked",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Inferred => "done?",
            Self::Lost => "lost",
            Self::Background => "waiting",
            Self::Stale => "stale",
            Self::Unknown(code) => code,
        }
    }

    /// Still going (Python `live`): anything but done, cancelled, inferred done and lost.
    pub fn is_live(&self) -> bool {
        !matches!(
            self,
            Self::Done | Self::Cancelled | Self::Inferred | Self::Lost
        )
    }

    /// The operator is needed (Python `render.needs`): a question, a permission prompt or a
    /// failure.
    pub fn needs_operator(&self) -> bool {
        matches!(self, Self::Waiting | Self::Blocked | Self::Failed)
    }

    /// The sort key, most urgent first (Python `urgency`): its place in the order, or after
    /// every listed status.
    pub fn urgency(&self) -> usize {
        ORDER.iter().position(|s| s == self).unwrap_or(ORDER.len())
    }
}

impl From<String> for Status {
    fn from(code: String) -> Self {
        Self::from_code(&code)
    }
}

impl From<Status> for String {
    fn from(st: Status) -> Self {
        st.code().to_owned()
    }
}

#[cfg(test)]
mod tests;
