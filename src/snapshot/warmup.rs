//! A one-shot CLI may briefly wait for initial Codex history. UI sources never
//! use this path; one deadline covers every pane and failed reads stop early.

use panorama_core::model::Swarm;
use std::time::{Duration, Instant};

/// Enough for normal local app-server startup, bounded independently of its
/// longer I/O timeout. A slow server leaves panes visible without details.
pub(super) const BUDGET: Duration = Duration::from_millis(750);
const POLL: Duration = Duration::from_millis(10);

pub(super) fn collect(mut refresh: impl FnMut() -> (Swarm, bool)) -> Swarm {
    let start = Instant::now();
    with_clock(&mut refresh, BUDGET, || start.elapsed(), std::thread::sleep)
}

fn with_clock(
    mut refresh: impl FnMut() -> (Swarm, bool),
    budget: Duration,
    elapsed: impl Fn() -> Duration,
    mut pause: impl FnMut(Duration),
) -> Swarm {
    let (mut swarm, mut pending) = refresh();
    while pending {
        let Some(remaining) = budget.checked_sub(elapsed()).filter(|left| !left.is_zero()) else {
            break;
        };
        pause(POLL.min(remaining));
        if elapsed() >= budget {
            break;
        }
        (swarm, pending) = refresh();
    }
    swarm
}

#[cfg(test)]
mod tests;
