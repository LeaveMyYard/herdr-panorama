//! The core ↔ panel protocol, viewer side (`docs/protocol/panel-v1.md`; Python
//! `protocol.py` and `panel.PanelClient`).
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

/// The protocol this viewer speaks: 1.1 the panels map and open, 1.2 focusing, 1.3 the
/// shared filter.
pub const PROTOCOL: &str = "1.3";

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

#[cfg(test)]
mod tests;
