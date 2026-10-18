//! Everything that moves, as a function of the animation time (Python `render.py`: `spin`,
//! `clock`, `blink`, `wave`, the unseen pulse and tint).
//!
//! `frame` counts design frames of 250 ms from the wall clock. Whole frames move glyphs
//! (spinner, clock); the fraction in between moves only colours (blinks, pulses). Python
//! renders at whole frames and restyles its `Anim` cells in between; here the view is simply
//! rendered again at the fractional frame, which gives the same cells (Python's own
//! "restyle equals a full render" property, `tests/test_smooth.py`).

use super::colour::Rgb;
use super::palette::{FAINT, status};
use super::unseen::Mark;
use crate::model::{Agent, CLOCK, SPIN, Status};
use std::f64::consts::PI;

/// One blink: on for a frame, an ease off, off for a frame, an ease back on.
pub const BLINK_PERIOD: f64 = 4.0;
/// One slow unseen pulse: 3 s.
pub const PULSE_PERIOD: f64 = 12.0;

/// The whole frame (Python `int(frame)`; frames are never negative).
fn whole(frame: f64) -> i64 {
    frame.floor() as i64
}

/// The running spinner's glyph: a step per whole frame, phase-shifted by `seq`.
pub fn spin(frame: f64, seq: i64) -> char {
    SPIN[(whole(frame) + seq).rem_euclid(SPIN.len() as i64) as usize]
}

/// Waiting: a dot circling the cell, a step per second (every 4 frames).
pub fn clock(frame: f64, seed: i64) -> char {
    CLOCK[(whole(frame).div_euclid(4) + seed).rem_euclid(CLOCK.len() as i64) as usize]
}

/// An agent's glyph now: the clock while waiting, the spinner while running, else its
/// still glyph.
pub fn glyph(a: &Agent, frame: f64) -> String {
    match a.status {
        Status::Background => clock(frame, a.seq).to_string(),
        Status::Running => spin(frame, a.seq).to_string(),
        _ => a.status.glyph().to_owned(),
    }
}

/// The attention blink, 1 on .. 0 off (whole frames hit exactly 1 or 0).
pub fn blink(frame: f64) -> f64 {
    let p = frame.rem_euclid(BLINK_PERIOD);
    if p <= 1.0 {
        1.0
    } else if p < 2.0 {
        (1.0 + (PI * (p - 1.0)).cos()) / 2.0
    } else if p <= 3.0 {
        0.0
    } else {
        (1.0 - (PI * (p - 3.0)).cos()) / 2.0
    }
}

/// `on` blinking between itself and `off`.
pub fn blink_between(frame: f64, on: Rgb, off: Rgb) -> Rgb {
    let k = blink(frame);
    if k >= 1.0 {
        on
    } else if k <= 0.0 {
        off
    } else {
        off.mix(on, k)
    }
}

/// `on` blinking against the faint colour (Python `blink_col`).
pub fn blink_faint(frame: f64, on: Rgb) -> Rgb {
    blink_between(frame, on, FAINT)
}

/// 0 .. 1 .. 0 over one pulse, a sine ease.
pub fn wave(frame: f64) -> f64 {
    (1.0 - (2.0 * PI * frame.rem_euclid(PULSE_PERIOD) / PULSE_PERIOD).cos()) / 2.0
}

/// The unseen pulse over `base`: full 35..100 %, quiet 15..45 % of the event's colour,
/// times `cap` (never fully off, so a still frame shows it).
pub fn pulse(frame: f64, m: Mark, base: Rgb, cap: f64) -> Rgb {
    let w = wave(frame);
    let k = if m.quiet {
        0.15 + 0.3 * w
    } else {
        0.35 + 0.65 * w
    };
    base.mix(status(&m.event.status()), k * cap)
}

/// The docked list's background tint for an unseen entry: 8..30 % (quiet 4..14 %).
pub fn tint(frame: f64, m: Mark, base: Rgb) -> Rgb {
    let w = wave(frame);
    let k = if m.quiet {
        0.04 + 0.1 * w
    } else {
        0.08 + 0.22 * w
    };
    base.mix(status(&m.event.status()), k)
}

#[cfg(test)]
mod tests;
