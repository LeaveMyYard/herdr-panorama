//! App feed v2 keeps runtime provenance beside the swarm (`docs/protocol/app-feed-v2.md`).
pub mod cli;
pub mod ensure;
pub mod fs;
pub mod inventory;
pub mod journal;
pub mod json;
pub mod liveness;
pub mod receipt;
pub mod resume;
pub mod runtime;
pub mod serialize;
pub mod start;
pub mod triggers;

use crate::host::{Host, collect::Collected};
use panorama_core::model::Swarm;
use std::collections::HashMap;

/// Address and reader-verified harness provenance for one root, in swarm order.
#[derive(Clone, Debug)]
pub struct Root {
    pub host: Host,
    pub terminal: Option<String>,
    pub agent_harnesses: HashMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct World {
    pub swarm: Swarm,
    pub roots: Vec<Root>,
    /// The optional agent-resume report; `None` while that feature is off.
    pub resume: Option<serde_json::Value>,
}

impl From<Collected> for World {
    fn from(collected: Collected) -> Self {
        let roots = collected
            .swarm
            .roots
            .iter()
            .map(|root| {
                // These readers know their own format; unsupported strings are never guessed.
                let reader = match root.harness.as_str() {
                    "claude" => "claude",
                    "codex" => "codex",
                    _ => "unknown",
                };
                Root {
                    host: collected.host,
                    terminal: collected.terminals.get(&root.pane_id).cloned(),
                    agent_harnesses: root
                        .agents
                        .iter()
                        .map(|a| (a.id.clone(), reader.into()))
                        .collect(),
                }
            })
            .collect();
        Self {
            swarm: collected.swarm,
            roots,
            resume: None,
        }
    }
}
