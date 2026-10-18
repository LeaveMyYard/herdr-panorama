//! A thread's phases (Python `render.cur_idx`, `phase_glyph`, `thread_glyph`, `phase_sub`,
//! `step_line`): which one is current, its glyph, the thread's subtitle, one step's line.

use crate::model::{PhaseState, Status, Thread, ThreadStatus};
use crate::view::anim::glyph;
use crate::view::colour::Rgb;
use crate::view::palette::{FAINT, FG, HI, status};
use crate::view::screen::Screen;
use crate::view::seg::Seg;
use crate::view::state::View;
use crate::view::text::{dur, fit, hhmm, trunc};

/// The current phase: the first now or failed one; the last when all are done or skipped;
/// else the first. `None` without phases.
pub fn cur_idx(t: &Thread) -> Option<usize> {
    if t.phases.is_empty() {
        return None;
    }
    if let Some(i) = t
        .phases
        .iter()
        .position(|p| matches!(p.state, PhaseState::Now | PhaseState::Failed))
    {
        return Some(i);
    }
    let finished = t
        .phases
        .iter()
        .all(|p| matches!(p.state, PhaseState::Done | PhaseState::Skipped));
    Some(if finished { t.phases.len() - 1 } else { 0 })
}

/// Phase `i`'s glyph and colour: done, skipped, next, failed; a current phase shows its most
/// urgent live agent's glyph (else the spinner's first frame).
pub fn phase_glyph(t: &Thread, i: usize, view: &View) -> (String, Rgb) {
    match t.phases[i].state {
        PhaseState::Done => return ("✓".into(), status(&Status::Done)),
        PhaseState::Skipped => return ("-".into(), status(&Status::Cancelled)),
        PhaseState::Next => return ("·".into(), FAINT),
        PhaseState::Failed => return ("✗".into(), status(&Status::Failed)),
        _ => {}
    }
    let here = t
        .agents
        .iter()
        .filter(|a| a.status.is_live() && a.phases.last() == Some(&(i as i64)));
    match here.min_by_key(|a| a.status.urgency()) {
        // an unknown status takes the running colour here (`ST.get(status, ST[RUN])`)
        Some(u) => {
            let col = if matches!(u.status, Status::Unknown(_)) {
                status(&Status::Running)
            } else {
                status(&u.status)
            };
            (glyph(u, view.frame), col)
        }
        None => ("⠋".into(), status(&Status::Running)),
    }
}

/// A thread's glyph in the graph: its current phase's, else done, failed or running.
pub fn thread_glyph(t: &Thread, view: &View) -> (String, Rgb) {
    if let Some(i) = cur_idx(t) {
        return phase_glyph(t, i, view);
    }
    if t.is_done() {
        ("✓".into(), status(&Status::Done))
    } else if t.status == ThreadStatus::Failed {
        ("✗".into(), status(&Status::Failed))
    } else {
        ("⠋".into(), status(&Status::Running))
    }
}

/// The thread's subtitle: its current phase, how long, and its percentage.
pub fn phase_sub(t: &Thread, view: &View) -> String {
    let Some(i) = cur_idx(t) else {
        return t.status.code().to_owned();
    };
    let p = &t.phases[i];
    let mut out = p.name.clone();
    match (p.start.filter(|s| *s != 0.0), p.end.filter(|e| *e != 0.0)) {
        (Some(s), _) if p.state == PhaseState::Now => {
            out += &format!(" · {}", dur(Some(view.now - s)))
        }
        (Some(s), Some(e)) => out += &format!(" · {}", dur(Some(e - s))),
        _ => {}
    }
    if let Some(pct) = t.pct {
        out += &format!(" · {pct}%");
    }
    out
}

/// Phase `i` as one line at `(x, y)`, `w` cells: glyph, name, when, and its note.
pub fn step_line(s: &mut Screen, x: i32, y: i32, t: &Thread, i: usize, w: i32, view: &View) {
    let p = &t.phases[i];
    let cur = matches!(p.state, PhaseState::Now | PhaseState::Failed);
    let later = matches!(p.state, PhaseState::Next | PhaseState::Skipped);
    let (g, gc) = phase_glyph(t, i, view);
    let at = |t: f64| hhmm(Some(t), view.utc_offset);
    let when = match (p.start.filter(|s| *s != 0.0), p.end.filter(|e| *e != 0.0)) {
        (Some(_), _) if later => String::new(),
        (Some(s), Some(e)) => format!("{}–{}", at(s), at(e)),
        (Some(s), None) => format!("since {} · {}", at(s), dur(Some(view.now - s))),
        (None, _) => String::new(),
    };
    let name_fg = if cur {
        HI
    } else if later {
        FAINT
    } else {
        FG
    };
    let segs = [
        Seg::new(g, gc).bold(),
        Seg::plain(" "),
        Seg::new(fit(&p.name, 13), name_fg).bold_if(cur),
        Seg::new(fit(&when, 20), if cur { FG } else { FAINT }),
    ];
    let xx = s.segs(x, y, &segs);
    if let Some(note) = &p.note
        && xx < x + w - 2
    {
        s.segs(
            xx,
            y,
            &[Seg::new(trunc(note, x + w - xx), crate::view::palette::DIM)],
        );
    }
}
