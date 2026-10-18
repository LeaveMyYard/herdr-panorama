//! The swarm tree (Python `model.py` dataclasses): a [`Swarm`] of [`Orchestrator`]s, each with
//! its [`Thread`]s, whose [`Phase`]s and [`Agent`]s the views draw.
//!
//! Field names, defaults and meanings are `model.py`'s; that file documents each field. The
//! types deserialize from the core's `state.swarm` (protocol 1.x). Unknown fields are
//! ignored, so a newer minor that adds fields still reads.
//!
//! Field types are read strictly, unlike Python's `swarm_from_wire`, which never checks
//! them: a `null` where the model has no `Option`, a wrong JSON type inside a field, or a
//! list item that is not an object rejects the whole state. So making a field nullable (or
//! widening its type) is a breaking change for Rust viewers, not an additive minor.

mod agent;
mod codes;
mod counts;

pub use agent::Agent;
pub use codes::{HerdrStatus, PhaseState, ThreadSource, ThreadStatus};
pub use counts::Counts;

use serde::{Deserialize, Serialize};

/// Background work a session or agent started and has not heard the end of (a command left
/// running in the background, a watch on something).
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct BgTask {
    /// Its id.
    pub id: String,
    /// The kind of background work, as the harness names it.
    pub kind: String,
    /// What it does, as its starter described it.
    #[serde(default)]
    pub description: String,
    /// When it started (POSIX seconds).
    #[serde(default)]
    pub started: Option<f64>,
}

/// One step of a thread's plan.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Phase {
    /// Its name.
    pub name: String,
    /// Done, now, next, failed or skipped.
    pub state: PhaseState,
    /// When it started.
    #[serde(default)]
    pub start: Option<f64>,
    /// When it ended.
    #[serde(default)]
    pub end: Option<f64>,
    /// A note on it.
    #[serde(default)]
    pub note: Option<String>,
}

/// A piece of work an orchestrator drives, with its plan and its agents.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Thread {
    /// Its id, unique in its orchestrator.
    pub id: String,
    pub name: String,
    /// Where it was found: explicit, workflow, ref, turn or session.
    pub source: ThreadSource,
    #[serde(default)]
    pub phases: Vec<Phase>,
    #[serde(default)]
    pub agents: Vec<Agent>,
    /// Active, done, failed or cancelled.
    #[serde(default)]
    pub status: ThreadStatus,
    #[serde(default)]
    pub started: Option<f64>,
    #[serde(default)]
    pub ended: Option<f64>,
    #[serde(default)]
    pub worktree: Option<String>,
    #[serde(default)]
    pub pct: Option<i64>,
    #[serde(default)]
    pub flavour: Option<String>,
}

impl Thread {
    /// Finished: done or cancelled (Python `Thread.done`; a failed thread is not).
    pub fn is_done(&self) -> bool {
        matches!(self.status, ThreadStatus::Done | ThreadStatus::Cancelled)
    }
}

/// An orchestrator: an agent session in a herdr pane, with the threads it drives.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Orchestrator {
    /// The herdr pane it runs in: its identity.
    pub pane_id: String,
    pub tab_id: Option<String>,
    pub tab_label: String,
    pub session_id: Option<String>,
    /// What herdr reports for the pane's agent.
    #[serde(default)]
    pub herdr_status: HerdrStatus,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub focused: bool,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default = "agents_flavour")]
    pub flavour: String,
    #[serde(default)]
    pub transcript: Option<String>,
    /// Blocked: the questions it asks, or the permission it wants, as the harness words them.
    #[serde(default)]
    pub pending: Vec<String>,
    /// Open inbox question ids of this session.
    #[serde(default)]
    pub inbox: Vec<i64>,
    #[serde(default)]
    pub inbox_texts: Vec<String>,
    #[serde(default)]
    pub threads: Vec<Thread>,
    /// Done threads older than the window.
    #[serde(default)]
    pub hidden_done: i64,
    /// The tab's position in its workspace, 1-based (herdr's tab number).
    #[serde(default)]
    pub tab_no: Option<i64>,
    /// A saved herdr SSH machine's label; `None`: this machine.
    #[serde(default)]
    pub machine: Option<String>,
    /// That machine's stable id.
    #[serde(default)]
    pub machine_id: Option<String>,
    /// The main session's background work in flight.
    #[serde(default)]
    pub bg: Vec<BgTask>,
    /// The newest transcript timestamp of the session or its agents.
    #[serde(default)]
    pub last_activity: Option<f64>,
}

fn agents_flavour() -> String {
    "agents".into()
}

impl Orchestrator {
    /// Every agent of every thread, in order (Python `Orchestrator.agents()`).
    pub fn agents(&self) -> impl Iterator<Item = &Agent> {
        self.threads.iter().flat_map(|t| t.agents.iter())
    }
}

/// Everything one collect saw.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Swarm {
    pub orchestrators: Vec<Orchestrator>,
    pub generated_at: f64,
    pub herdr_ok: bool,
    pub inbox_ok: bool,
    /// Sessions or records that could not be read.
    pub errors: Vec<String>,
    /// The orchestrator pane a docked panel sits next to. Not on the wire: each viewer sets
    /// it (a per-tab panel: its own tab's orchestrator).
    #[serde(skip)]
    pub docked: Option<String>,
    /// Idle this long (seconds) reads "stale" (config `stale_after`). Missing, `null` or 0
    /// on the wire: the default, as Python's `or` reads it.
    #[serde(deserialize_with = "stale_after_or_default")]
    pub stale_after: f64,
}

fn stale_after_or_default<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    let v = Option::<f64>::deserialize(d)?;
    Ok(v.filter(|s| *s != 0.0).unwrap_or(STALE_AFTER))
}

/// Python's default `stale_after`: 12 hours.
pub const STALE_AFTER: f64 = 12.0 * 3600.0;

impl Default for Swarm {
    fn default() -> Self {
        Self {
            orchestrators: Vec::new(),
            generated_at: 0.0,
            herdr_ok: true,
            inbox_ok: false,
            errors: Vec::new(),
            docked: None,
            stale_after: STALE_AFTER,
        }
    }
}

#[cfg(test)]
mod tests;
