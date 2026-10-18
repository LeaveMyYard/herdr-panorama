//! The waits between connections (Python `BACKOFF` and `PanelClient._run`): 0.25, 0.5, 1, 2,
//! then every 5 s; a session that got its welcome starts the schedule over.

use std::time::Duration;

/// Python's `BACKOFF`: the waits before reconnecting, the last one repeating.
pub const BACKOFF: [Duration; 5] = [
    Duration::from_millis(250),
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(5),
];

/// Where the client is in its schedule.
#[derive(Clone, Debug)]
pub struct Backoff {
    schedule: Vec<Duration>,
    tries: usize,
}

impl Backoff {
    /// A schedule; an empty one is [`BACKOFF`].
    pub fn new(schedule: &[Duration]) -> Self {
        let schedule = if schedule.is_empty() {
            BACKOFF.to_vec()
        } else {
            schedule.to_vec()
        };
        Self { schedule, tries: 0 }
    }

    /// The wait after a connection ended; `welcomed`: it got as far as the welcome.
    pub fn next(&mut self, welcomed: bool) -> Duration {
        if welcomed {
            self.tries = 0;
        }
        let wait = self.schedule[self.tries.min(self.schedule.len() - 1)];
        self.tries += 1;
        wait
    }
}

#[cfg(test)]
mod tests;
