//! A viewer's outbox (Python `Viewer.send`): bounded; when full, the oldest message goes (a
//! stale state), so a slow viewer never stalls the core.

use std::collections::VecDeque;
use std::sync::Arc;

/// The queued lines, at most `cap`.
#[derive(Debug)]
pub struct Outbox {
    cap: usize,
    lines: VecDeque<Arc<Vec<u8>>>,
}

impl Outbox {
    pub fn new(cap: usize) -> Self {
        Self {
            cap: cap.max(1),
            lines: VecDeque::new(),
        }
    }

    /// Queues `line`, dropping the oldest while full.
    pub fn push(&mut self, line: Arc<Vec<u8>>) {
        while self.lines.len() >= self.cap {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    pub fn pop(&mut self) -> Option<Arc<Vec<u8>>> {
        self.lines.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

#[cfg(test)]
mod tests;
