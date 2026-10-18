//! Agents on herdr's saved SSH machines, next to the local ones (Python `machines.py`; see
//! `docs/REMOTE.md`). Lives in panorama for now, its only user (plan Q7); it moves to the
//! kit's `remote_machines` when the inbox needs it.
//!
//! Per machine, a background thread ([`Watcher`]) runs on its own cadence and never blocks
//! the view:
//!
//! 1. herdr state through herdr's own forwarding, `herdr --machine <id> api snapshot`
//!    (read-only; forwarding never starts a remote server);
//! 2. the Claude transcripts of the sessions that snapshot names, mirrored with `rsync` over
//!    ssh into the plugin state dir ([`mirror`], [`sync`]; append-only, so each refresh copies
//!    only new bytes), and pruned when the machine stops naming them ([`prune`]);
//! 3. the same collector as local sessions, pointed at that mirror ([`Collect`]); then every
//!    orchestrator is tagged with its machine and its herdr ids are namespaced
//!    (`<machine id>:w1:p2`, [`namespace`]), because ids are per server.
//!
//! [`Remotes`] merges the latest result of every machine into the local swarm. A machine
//! that fails keeps its last result, marked stale; one that never answered shows as
//! unreachable. Which machines: the `machines` setting ([`select`]).
//!
//! Python logs a skipped unsafe ssh target and a failed transcript sync; this port does not
//! log (the binary has no log yet). The sync error still reaches the view in `errors`.

pub mod mirror;
pub mod namespace;
pub mod prune;
pub mod remotes;
pub mod select;
pub mod sync;
pub mod watcher;

pub use namespace::{local_id, namespace, prefix};
pub use remotes::Remotes;
pub use select::{Selection, Source, saved};
pub use watcher::{Collect, Outcome, Refresher, SnapshotApi, StopFlag, Watcher};

use herdr_agent_kit::agent_session::SessionId;
use std::time::Duration;

/// A machine's refresh cadence (its own thread).
pub const REMOTE_SECONDS: Duration = Duration::from_secs(5);
/// Continuation hops mirrored within one refresh (the rest: the next one).
pub const CONTINUE_ROUNDS: usize = 3;
/// A result older than this is shown as stale.
pub const STALE_SECONDS: Duration = Duration::from_secs(30);
/// The longest error text a machine adds to the view's errors, in characters.
pub const ERROR_CHARS: usize = 200;

/// One of herdr's saved SSH machines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Machine {
    /// Its stable id: the namespace of its herdr ids.
    pub id: String,
    /// Its label, for display (labels can repeat and contain `:`).
    pub label: String,
    /// The ssh target of the saved profile.
    pub target: String,
}

impl Machine {
    /// A machine from its id, label and ssh target.
    pub fn new(id: &str, label: &str, target: &str) -> Self {
        Self {
            id: id.to_owned(),
            label: label.to_owned(),
            target: target.to_owned(),
        }
    }
}

/// A Claude Code session id: a UUID, the whole string, any case. Anything else never becomes
/// an rsync rule or a path to delete (it could be empty, `..` or carry a separator).
pub fn valid_session(sid: &str) -> bool {
    SessionId::parse(sid).is_some()
}

/// An ssh target that does not read as an option itself: not empty, no leading `-`, no
/// whitespace (as Python's `isspace` counts it: `\x1c`..`\x1f` too). As in Python, a
/// `user@-host` target passes; rsync's own host check refuses it ("hostnames may not start
/// with '-'").
pub fn safe_target(target: &str) -> bool {
    let space = |c: char| c.is_whitespace() || ('\x1c'..='\x1f').contains(&c);
    !target.is_empty() && !target.starts_with('-') && !target.chars().any(space)
}

/// `text` cut to [`ERROR_CHARS`] characters (Python `[:200]`).
pub(crate) fn cut(text: String) -> String {
    match text.char_indices().nth(ERROR_CHARS) {
        Some((at, _)) => text[..at].to_owned(),
        None => text,
    }
}

#[cfg(test)]
mod tests;
