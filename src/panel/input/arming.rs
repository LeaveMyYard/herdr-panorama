//! When a click acts (Python `SwarmApp.arrived`, `scrolled`): a cell whose click target
//! changed in a refresh ignores clicks for [`CLICK_ARM`] (a click aimed before the refresh
//! must not land on something else); one the wheel slid under a still pointer, for
//! [`SCROLL_ARM`] (trackpad momentum).

use panorama_core::view::{Screen, Target};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A cell whose target appeared or moved in a refresh.
pub const CLICK_ARM: Duration = Duration::from_secs(1);
/// A cell whose target the wheel moved under the pointer.
pub const SCROLL_ARM: Duration = Duration::from_millis(300);

type Cells = HashMap<(i32, i32), Target>;

/// What a click on each cell would hit (the last target registered wins).
pub fn cells(s: &Screen) -> Cells {
    let mut out = HashMap::new();
    for t in &s.targets {
        for x in t.x0..t.x1 {
            out.insert((x, t.y), t.target.clone());
        }
    }
    out
}

/// The cells that ignore clicks, until when.
#[derive(Debug, Default)]
pub struct ClickArming {
    arrived: HashMap<(i32, i32), Instant>,
    scrolled: HashMap<(i32, i32), Instant>,
}

impl ClickArming {
    /// After a refresh painted `new` over `old`: every cell whose target changed waits.
    pub fn refreshed(&mut self, old: &Screen, new: &Screen, now: Instant) {
        self.arrived
            .retain(|_, at| now.duration_since(*at) < CLICK_ARM);
        let before = cells(old);
        for (cell, hit) in cells(new) {
            if before.get(&cell) != Some(&hit) {
                self.arrived.insert(cell, now);
            }
        }
    }

    /// After the wheel moved `old` to `new`: every cell whose target changed (or went) waits
    /// a little.
    pub fn scrolled(&mut self, old: &Screen, new: &Screen, now: Instant) {
        self.scrolled.retain(|_, until| *until > now);
        let (before, after) = (cells(old), cells(new));
        for (cell, hit) in &after {
            if before.get(cell) != Some(hit) {
                self.scrolled.insert(*cell, now + SCROLL_ARM);
            }
        }
        for cell in before.keys().filter(|c| !after.contains_key(c)) {
            self.scrolled.insert(*cell, now + SCROLL_ARM);
        }
    }

    /// Whether a click on `cell` acts now.
    pub fn ready(&self, cell: (i32, i32), now: Instant) -> bool {
        let fresh = self
            .arrived
            .get(&cell)
            .is_some_and(|at| now.duration_since(*at) < CLICK_ARM);
        let sliding = self.scrolled.get(&cell).is_some_and(|until| now < *until);
        !fresh && !sliding
    }
}

/// What caused a repaint, for the click arming (Python stamps cells in `apply` and in
/// `wheel`, never on a frame or a hover).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Painted {
    /// A state from the core.
    Refresh,
    /// The wheel scrolled a column.
    Wheel,
    /// An animation frame, a hover, a key: no cell is stamped.
    Other,
}

impl crate::panel::model::Panel {
    /// The actor repainted, `old` (the screen before, if any) to `new`, because of `cause`:
    /// the cells whose click target changed wait before a click acts there. The actor calls
    /// it after every paint; only a refresh or a wheel stamps cells.
    pub fn painted(&mut self, old: Option<&Screen>, new: &Screen, cause: Painted, now: Instant) {
        let Some(old) = old else { return };
        match cause {
            Painted::Refresh => self.input.clicks.refreshed(old, new, now),
            Painted::Wheel => self.input.clicks.scrolled(old, new, now),
            Painted::Other => {}
        }
    }
}

#[cfg(test)]
mod tests;
