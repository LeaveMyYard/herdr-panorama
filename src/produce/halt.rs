//! Stopping the remote watchers from outside the producer (quitting while a refresh runs):
//! the stop flags of the watchers it runs, kept current by each refresh.

use crate::machines::{Remotes, StopFlag};
use std::sync::{Arc, Mutex, PoisonError};

#[derive(Debug, Default)]
struct State {
    stopped: bool,
    watchers: Vec<StopFlag>,
}

/// The producer's halt: [`Halt::stop`] stops every watcher it runs now, and any it starts
/// later at once. Cloned to the thread that quits.
#[derive(Clone, Debug, Default)]
pub struct Halt(Arc<Mutex<State>>);

impl Halt {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Stop every watcher, now and from now on (a sync under way is killed: its rsync's own
    /// process group). Does not wait for their threads.
    pub fn stop(&self) {
        let mut st = self.state();
        st.stopped = true;
        for w in &st.watchers {
            w.stop();
        }
    }

    /// Whether [`Halt::stop`] was called.
    pub fn is_stopped(&self) -> bool {
        self.state().stopped
    }

    /// The watchers `remotes` runs now; stopped at once after a halt.
    pub(super) fn track(&self, remotes: &Remotes) {
        let mut st = self.state();
        st.watchers = remotes.watchers.iter().map(|w| w.stop_flag()).collect();
        if st.stopped {
            for w in &st.watchers {
                w.stop();
            }
        }
    }
}
