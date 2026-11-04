//! The core ↔ panel protocol, viewer side (`docs/protocol/panel-v2.md`; Python's
//! `protocol.py` and `panel.PanelClient` speak 1.x, its legacy threads view).
//!
//! Newline-delimited JSON over the core's Unix socket, one object per line. `protocol` is
//! "MAJOR.MINOR": a peer with another major is refused, a minor only adds fields, and
//! unknown fields and messages are ignored.
//!
//! - [`framing`]: lines in and out, with the 4 MiB limit.
//! - [`messages`]: what a viewer sends and what the core answers.
//! - [`state`]: the core's `state` snapshot, read as leniently as Python's panel reads it.
//! - [`socket`]: where the core listens, and that it is ours.
//! - [`client`]: the viewer's connection, reconnecting, as a task.

pub mod client;
pub mod framing;
pub mod messages;
pub mod socket;
pub mod state;

/// The protocol this viewer speaks. 2.0: `state.swarm` is the uniform agent tree (`roots`,
/// each with one flat `agents` list; no threads or phases). Before it, 1.1 the panels map and
/// open, 1.2 focusing, 1.3 the shared filter, 1.4 the agents' and roots' titles.
pub const PROTOCOL: &str = "2.0";

/// A longer line is refused and the connection closed.
pub const MAX_LINE: usize = 4 * 1024 * 1024;

/// The major of a `"MAJOR.MINOR"` version, if it has one.
pub fn major(version: &str) -> Option<i64> {
    version.split('.').next()?.trim().parse().ok()
}

/// A peer of this version can be talked to: the same major.
pub fn compatible(version: &str) -> bool {
    major(version).is_some() && major(version) == major(PROTOCOL)
}

/// An open key (a `hello` with `"role": "key"`) of this version can be answered: the same
/// major, or 1. A key connection sends `open` and reads `opened`, which 2.0 left as they were,
/// and it gets no `state`; herdr's open action runs the Python `panorama open`, which says
/// 1.x, also when the core is this one.
pub fn key_compatible(version: &str) -> bool {
    compatible(version) || major(version) == Some(1)
}

#[cfg(test)]
mod tests;
