//! The operator looked at these agents (Python `SwarmApp._seen`): the producer's tracker
//! clears their marks at once, so the next refresh does not bring them back, and the state
//! file is written off the UI thread. The view cleared them already ([`crate::ui::input::Host::seen`]).

use super::collect::THREAD;
use crate::unseen::Tracker;
use std::sync::Arc;
use std::thread::JoinHandle;

/// Clears `ids` in `tracker`; the ones that had a mark are saved on a thread of their own
/// (its handle; `None` when none had one). A save that fails, or finds the file's lock busy,
/// leaves them pending for the next refresh.
pub fn mark(tracker: &Arc<Tracker>, ids: &[String]) -> Option<JoinHandle<()>> {
    let cleared = tracker.seen(ids.iter().map(String::as_str));
    if cleared.is_empty() {
        return None;
    }
    let t = tracker.clone();
    std::thread::Builder::new()
        .name(THREAD.into())
        .spawn(move || t.save_seen(&cleared))
        .ok()
}

#[cfg(test)]
mod tests;
