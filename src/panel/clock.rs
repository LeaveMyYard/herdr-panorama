//! The panel's clocks: the wall clock (ages, the clock on the empty page) and the animation
//! time, `now / 0.25 s` (Python `SwarmApp.anim_frame`): a pure function of the time of day, so
//! every panorama window shows an animation in the same phase, however long it was hidden.

use std::time::{SystemTime, UNIX_EPOCH};

/// One design frame: spinners turn and sweeps move one step.
pub const FRAME_SECONDS: f64 = 0.25;

/// Now, in POSIX seconds.
pub fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

/// The animation time at `now`, in frames.
pub fn frame_at(now: f64) -> f64 {
    now / FRAME_SECONDS
}

/// The local time zone's offset now, seconds east of UTC.
pub fn utc_offset() -> i64 {
    i64::from(chrono::Local::now().offset().local_minus_utc())
}

/// The local time for a crash log's first line, `YYYY-MM-DD HH:MM:SS +zzzz`.
pub fn stamp() -> String {
    chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S %z")
        .to_string()
}

#[cfg(test)]
mod tests;
