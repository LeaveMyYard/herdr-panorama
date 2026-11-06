//! The swarm tree (plan `docs/plans/uniform-agent-tree.md` §2.1): a [`Swarm`] of [`Root`]s,
//! each with its [`Agent`]s, one flat tree (parents before children, `parent: None` a
//! direct child of the root).
//!
//! Field names, defaults and meanings started as Python `model.py`'s; Python's model still
//! has threads (its legacy view, plan §8 D7), this one does not. The types deserialize from
//! the core's `state.swarm` (protocol 2.x). Unknown fields are ignored, so a newer minor
//! that adds fields still reads.
//!
//! Field types are read strictly, unlike Python's `swarm_from_wire`, which never checks
//! them: a `null` where the model has no `Option`, a wrong JSON type inside a field, or a
//! list item that is not an object rejects the whole state. So making a field nullable (or
//! widening its type) is a breaking change for Rust viewers, not an additive minor.

mod agent;
mod codes;
mod counts;

pub use agent::Agent;
pub use codes::HerdrStatus;
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

/// A root: an agent session in a herdr pane, the depth-0 node of its agent tree. Its own
/// state (herdr status, pending prompt, inbox questions no agent filed, background work) is
/// on it; everything it spawned, at any depth, is in [`Self::agents`].
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Root {
    /// The herdr pane it runs in: its identity.
    pub pane_id: String,
    /// The harness reported by herdr. Older states contain only Claude sessions.
    #[serde(default = "claude_harness")]
    pub harness: String,
    pub tab_id: Option<String>,
    pub tab_label: String,
    /// What the views call it (plan §2.2): the operator's herdr name, else the session's
    /// title, first prompt or telling terminal title, else its tab or pane. Empty from a
    /// producer that predates titles: views fall back to `tab_label` ([`Self::name`]).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// Which rule gave the title (`pane`, `tab`, `session`, `prompt`, `id`); drives no logic.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title_source: String,
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
    /// Every agent it spawned, at any depth, flat: parents before children, `parent: None`
    /// a direct child of the root (plan `uniform-agent-tree.md` §2.1).
    #[serde(default)]
    pub agents: Vec<Agent>,
    /// Finished agents (done, failed, cancelled) older than the done window, left out of
    /// [`Self::agents`] (plan §2.4).
    #[serde(default)]
    pub hidden_finished: i64,
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

fn claude_harness() -> String {
    "claude".into()
}

impl Root {
    /// The name the views show: its title, else (an older producer) its tab label.
    pub fn name(&self) -> &str {
        if self.title.is_empty() {
            &self.tab_label
        } else {
            &self.title
        }
    }
}

/// Everything one collect saw.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Swarm {
    pub roots: Vec<Root>,
    pub generated_at: f64,
    pub herdr_ok: bool,
    pub inbox_ok: bool,
    /// Sessions or records that could not be read.
    pub errors: Vec<String>,
    /// The root pane a docked panel sits next to. Not on the wire: each viewer sets it (a
    /// per-tab panel: its own tab's root).
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
            roots: Vec::new(),
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
