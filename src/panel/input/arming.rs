//! When a click acts: the shared rules are [`crate::ui::input::arming`]'s; the panel stamps
//! its cells after each paint here.

pub use crate::ui::input::arming::*;
use panorama_core::view::Screen;
#[cfg(test)]
use panorama_core::view::Target;
#[cfg(test)]
use std::time::Duration;
use std::time::Instant;

impl crate::panel::model::Panel {
    /// The actor repainted, `old` (the screen before, if any) to `new`, because of `cause`:
    /// the cells whose click target changed wait before a click acts there. The actor calls
    /// it after every paint; only a refresh or a wheel stamps cells.
    /// No screen before (`old` `None`, the first paint) counts as one with no targets: every
    /// cell of the first screen waits, as Python's first `apply` stamps them all.
    pub fn painted(&mut self, old: Option<&Screen>, new: &Screen, cause: Painted, now: Instant) {
        let none = Screen::new(0, 0);
        let old = old.unwrap_or(&none);
        match cause {
            Painted::Refresh => self.input.clicks.refreshed(old, new, now),
            Painted::Wheel => self.input.clicks.scrolled(old, new, now),
            Painted::Other => {}
        }
    }
}

#[cfg(test)]
mod tests;
