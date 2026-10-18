//! When the panel draws an animation frame (Python `SwarmApp.animate`): only while its tab is
//! shown and something on it moves, and then only when the frame would differ from the one
//! on screen: a new whole frame (spinners step), or any time while a colour glides (pulses,
//! blinks).

use panorama_core::model::Swarm;
use panorama_core::view::moving::animated;
use panorama_core::view::{Marks, Screen};

/// Whether the colour tick has anything to do: the tab is shown and something on it moves.
/// When not, the panel does not even wake for the tick.
pub fn ticking(tab_shown: bool, swarm: &Swarm, unseen: &Marks) -> bool {
    tab_shown && animated(swarm, unseen)
}

/// What is on screen, for deciding whether a tick draws.
#[derive(Debug, Default)]
pub struct Pacing {
    /// The whole frame last drawn.
    drawn: Option<i64>,
    /// That screen has colours that glide between whole frames.
    glides: bool,
}

impl Pacing {
    /// Whether a tick at animation time `frame` draws a new frame.
    pub fn due(&self, frame: f64) -> bool {
        self.glides || self.drawn != Some(frame.floor() as i64)
    }

    /// `screen` was drawn at `frame`.
    pub fn drawn(&mut self, frame: f64, screen: &Screen) {
        self.drawn = Some(frame.floor() as i64);
        self.glides = glides(screen);
    }
}

/// Whether a screen has colours that glide between whole frames (pulses, blinks).
fn glides(s: &Screen) -> bool {
    (0..s.h()).any(|y| {
        (0..s.w()).any(|x| {
            let c = s.cell(x, y);
            c.fg_moves || c.bg_moves
        })
    })
}

#[cfg(test)]
mod tests;
