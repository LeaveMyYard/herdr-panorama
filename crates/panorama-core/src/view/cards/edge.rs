//! A card's animated edge (Python `render.edge`, `sweep_pos`): the left border in the state's
//! colour (blinking when it needs the operator), and while agents work a 4-cell bar in the
//! running colour sweeping the bottom border.

use super::Area;
use crate::model::Status;
use crate::view::anim::blink_faint;
use crate::view::orch_state::OrchState;
use crate::view::palette::status;
use crate::view::screen::Screen;
use crate::view::seg::Seg;

/// Frames of 250 ms per lap: 12 s, the lap of a 44-column card (wider cards move further).
pub const SWEEP_LAP: i64 = 48;

/// The first cell of the bar on an `n`-cell bottom border (may be off either end).
pub fn sweep_pos(frame: f64, seed: i64, n: i64) -> i64 {
    let lap = (n + 6).min(SWEEP_LAP);
    ((frame.floor() as i64 + seed * 7).rem_euclid(lap)) * (n + 6) / lap - 3
}

fn quiet(st: &Status) -> bool {
    matches!(
        st,
        Status::Queued
            | Status::Done
            | Status::Cancelled
            | Status::Inferred
            | Status::Lost
            | Status::Stale
    )
}

/// The edge of `card` in state `st`; `seed` shifts its sweep so cards do not move in
/// lockstep.
pub fn edge(cv: &mut Screen, card: Area, st: &OrchState, frame: f64, seed: i64) {
    let Area { x, y, w, h } = card;
    if quiet(&st.st) {
        return;
    }
    let attention = st.st.needs_operator();
    let col = if attention {
        blink_faint(frame, status(&st.st))
    } else {
        status(&st.st)
    };
    let side = Seg::new("┃", col).fg_moving(attention);
    for j in 1..h - 1 {
        cv.put_seg(x, y + j, &side);
    }
    let bottom = y + h - 1;
    if !st.working || !(0..cv.h()).contains(&bottom) {
        return;
    }
    let n = i64::from(w - 2);
    let pos = sweep_pos(frame, seed, n);
    for i in 0..n {
        // every cell of the bottom border moves with the bar (Python's Anim cells)
        if let Some(c) = cv.cell_mut(x + 1 + i as i32, bottom) {
            if (0..4).contains(&(i - pos)) {
                c.ch = '━';
                c.fg = status(&Status::Running);
            }
            c.fg_moves = true;
            c.bold = false;
        }
    }
}
