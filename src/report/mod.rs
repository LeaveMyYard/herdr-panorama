//! The explicit thread channel, write side (Python `events.emit`, `_compact`, `report`):
//! what `report_thread` (and `panorama emit`) append to a session's events file. The read side
//! is [`crate::collect::events`].
//!
//! - [`emit`]: one declaration line, appended under a lock beside the file; past
//!   [`COMPACT_AT`] the file is rewritten as one full declaration per thread.
//! - [`merge`]: an agent's report merged into its thread's declaration: phases keep their
//!   start and end times across calls.
//!
//! Stated deviation: a phase's keys are written in sorted order (Python keeps the order they
//! were first written in); readers of both languages read them by name.

pub mod emit;
pub mod merge;

pub use emit::{compact, emit};
pub use merge::{Report, Reported, report};

/// Phases and agents per thread (and per report list).
pub const MAX_ITEMS: usize = 30;
/// The longest text of a reported name, in characters.
pub const MAX_TEXT: usize = 200;
/// Past this size the events file is compacted.
pub const COMPACT_AT: u64 = 64 * 1024;

/// `time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(t))`.
pub fn stamp(t: f64) -> String {
    chrono::DateTime::from_timestamp(t.trunc() as i64, 0)
        .map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_default()
}
