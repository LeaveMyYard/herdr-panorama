//! One rsync run, bounded (Python `machines._run`): in its own process group, so a timeout
//! kills rsync and its ssh together ([`super::bounded`]).

use super::bounded::{Ended, bounded};
use std::time::Duration;

/// The longest error text kept from rsync's stderr, in characters.
const ERROR_CHARS: usize = 300;

/// The text of a run its watcher stopped.
pub const STOPPED: &str = "rsync stopped";

/// The text of a timeout.
pub const TIMED_OUT: &str = "rsync timed out";

/// rsync's exit 24: some source files vanished while it ran (Claude Code rewrote them).
const VANISHED: i32 = 24;

/// Runs `argv`; `None` when it succeeded (exit 0 or 24), else the error: its stderr (trimmed,
/// at most 300 characters), `rsync exit <n>`, [`TIMED_OUT`], [`STOPPED`] when `cancelled()`
/// said so while it ran, or why it could not start.
///
/// Like Python's `communicate()`, a run that exited waits for stderr to close (here never
/// past the deadline): a grandchild holding it keeps the run open that long.
///
/// Deviation, stated: a failure whose stderr is only whitespace reads `rsync exit <n>`; Python
/// returns that empty text, which its caller takes for success.
pub fn run(argv: &[String], timeout: Duration, cancelled: &dyn Fn() -> bool) -> Option<String> {
    let (code, stderr) = match bounded(argv, timeout, false, cancelled) {
        Ended::Exited { code, stderr, .. } => (code, stderr),
        Ended::TimedOut => return Some(TIMED_OUT.to_owned()),
        Ended::Cancelled => return Some(STOPPED.to_owned()),
        Ended::NotStarted(e) => return Some(format!("rsync: {e}")),
    };
    if matches!(code, Some(0 | VANISHED)) {
        return None;
    }
    let err = String::from_utf8_lossy(&stderr);
    let text = match err.trim() {
        "" => format!("rsync exit {}", code.unwrap_or_default()),
        t => t.to_owned(),
    };
    Some(text.chars().take(ERROR_CHARS).collect())
}

#[cfg(test)]
mod tests;
