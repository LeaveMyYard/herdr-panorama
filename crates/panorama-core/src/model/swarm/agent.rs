//! One agent of a thread (Python `model.Agent`), with `model.py`'s defaults.

use super::BgTask;
use crate::model::Status;
use serde::{Deserialize, Serialize};

/// A subagent (or, with an id starting `main:`, the session's own work in a thread).
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Agent {
    /// Its id, unique in the swarm.
    pub id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "agent_role")]
    pub role: String,
    #[serde(default)]
    pub agent_type: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default = "running")]
    pub status: Status,
    #[serde(default)]
    pub tool_use_id: Option<String>,
    #[serde(default)]
    pub started: Option<f64>,
    #[serde(default)]
    pub ended: Option<f64>,
    #[serde(default)]
    pub last_at: Option<f64>,
    #[serde(default)]
    pub tool_calls: i64,
    #[serde(default)]
    pub activity: String,
    #[serde(default)]
    pub last_message: String,
    /// Open inbox question texts.
    #[serde(default)]
    pub questions: Vec<String>,
    /// A blocking prompt: the questions it asks, or the permission it wants.
    #[serde(default)]
    pub pending: Vec<String>,
    #[serde(default)]
    pub question_ids: Vec<i64>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub phases: Vec<i64>,
    /// The agent that spawned it, if any.
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default = "one")]
    pub depth: i64,
    #[serde(default)]
    pub resumes: i64,
    /// Its status was guessed from its own file.
    #[serde(default)]
    pub inferred: bool,
    #[serde(default)]
    pub transcript: Option<String>,
    /// A stable index, for the spinner's phase.
    #[serde(default)]
    pub seq: i64,
    /// Its own background work in flight.
    #[serde(default)]
    pub bg: Vec<BgTask>,
}

fn agent_role() -> String {
    "agent".into()
}

fn running() -> Status {
    Status::Running
}

fn one() -> i64 {
    1
}

impl Default for Agent {
    fn default() -> Self {
        Self {
            id: String::new(),
            description: String::new(),
            role: agent_role(),
            agent_type: None,
            model: None,
            status: running(),
            tool_use_id: None,
            started: None,
            ended: None,
            last_at: None,
            tool_calls: 0,
            activity: String::new(),
            last_message: String::new(),
            questions: Vec::new(),
            pending: Vec::new(),
            question_ids: Vec::new(),
            error: None,
            phases: Vec::new(),
            parent: None,
            depth: one(),
            resumes: 0,
            inferred: false,
            transcript: None,
            seq: 0,
            bg: Vec::new(),
        }
    }
}
