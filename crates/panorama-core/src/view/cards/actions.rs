//! A card's action hints (Python `render.actions`): `g open pane`, `t transcript`, and
//! `a answer` when there is something to answer; each enabled hint is a click target. An
//! agent's and a root's cards draw the same row ([`hint_row`]).

use crate::model::{Agent, Status};
use crate::view::palette::{FAINT, FG, KEY};
use crate::view::screen::Screen;
use crate::view::seg::Seg;
use crate::view::target::{AgentRef, Target};

/// One hint of a row: whether it is enabled, and what a click on it hits (none: drawn only).
pub type Hint = (bool, Option<Target>);

/// `g open pane`, `t transcript` and `a <answer>` from `(x, y)`; `room`: the width they must
/// fit in (a narrow card closes the gaps, then says just "answer").
pub fn hint_row(
    s: &mut Screen,
    (x, y): (i32, i32),
    hints: [Hint; 3],
    answer: &str,
    room: Option<i32>,
) {
    let width = |gap: i32, answer: &str| 25 + answer.chars().count() as i32 + 2 * gap;
    let (gap, label) = match room {
        None => (3, answer),
        Some(room) => [(3, answer), (2, answer)]
            .into_iter()
            .find(|(g, a)| width(*g, a) <= room)
            .unwrap_or((2, "answer")),
    };
    let mut xx = x;
    for ((k, lab), (on, target)) in [("g", "open pane"), ("t", "transcript"), ("a", label)]
        .into_iter()
        .zip(hints)
    {
        let start = xx;
        let segs = [
            Seg::new(k, if on { KEY } else { FAINT }).bold(),
            Seg::new(format!(" {lab}"), if on { FG } else { FAINT }),
        ];
        xx = s.segs(xx, y, &segs) + gap;
        if let (true, Some(t)) = (on, target) {
            s.target(start, xx - gap, y, t);
        }
    }
}

/// An agent's hints from `(x, y)`; `ident`: the agent they act on; `room`: as [`hint_row`].
pub fn actions(
    s: &mut Screen,
    x: i32,
    y: i32,
    a: &Agent,
    inbox_ok: bool,
    ident: Option<&AgentRef>,
    room: Option<i32>,
) {
    let can = (a.status == Status::Waiting && inbox_ok && !a.question_ids.is_empty())
        || a.status == Status::Blocked;
    let answer = if a.status == Status::Blocked {
        "answer in pane"
    } else {
        "answer"
    };
    let hints = [
        (true, ident.map(|r| Target::Goto(r.clone()))),
        (true, ident.map(|r| Target::Transcript(r.clone()))),
        (can, ident.map(|r| Target::Answer(r.clone()))),
    ];
    hint_row(s, (x, y), hints, answer, room);
}
