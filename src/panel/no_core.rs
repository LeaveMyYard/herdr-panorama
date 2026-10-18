//! The panel without a core (Python `PanelApp.no_core_since`): the clock starts at launch
//! and whenever a connection ends, stops with each welcome; 10 minutes on it and the panel
//! exits (the core crashed, or per-tab went off while it was down), so no panel is left
//! behind for good. The next core makes them again.

use std::time::{Duration, Instant};

/// No core this long: the viewer exits.
pub const NO_CORE: Duration = Duration::from_secs(600);
/// How often that is looked at.
pub const NO_CORE_CHECK: Duration = Duration::from_secs(10);

/// Since when there has been no core.
#[derive(Debug)]
pub struct NoCoreClock {
    since: Option<Instant>,
}

impl NoCoreClock {
    /// Started at launch: no core yet.
    pub fn started(now: Instant) -> Self {
        Self { since: Some(now) }
    }

    /// A core welcomed the panel (`on`), or the connection ended: the clock stops, or starts
    /// if it was not running.
    pub fn connected(&mut self, on: bool, now: Instant) {
        self.since = if on { None } else { self.since.or(Some(now)) };
    }

    /// Whether the panel has been without a core for [`NO_CORE`].
    pub fn expired(&self, now: Instant) -> bool {
        self.since.is_some_and(|t| now.duration_since(t) >= NO_CORE)
    }
}

#[cfg(test)]
mod tests;
