//! An agent's action hints (Python `render.actions`): `g open pane`, `t transcript`, and
//! `a answer` when there is something to answer; each enabled hint is a click target.

use crate::model::{Agent, Status};
use crate::view::palette::{FAINT, FG, KEY};
use crate::view::screen::Screen;
use crate::view::seg::Seg;
use crate::view::target::{AgentRef, Target};

/// The hints from `(x, y)`; `ident`: the agent they act on; `room`: the width they must fit
/// in (a narrow card closes the gaps, then says just "answer").
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
    let mut label = if a.status == Status::Blocked {
        "answer in pane"
    } else {
        "answer"
    };
    let mut gap = 3;
    let width = |gap: i32, label: &str| {
        [("g", "open pane"), ("t", "transcript"), ("a", label)]
            .iter()
            .map(|(k, l)| (k.chars().count() + 1 + l.chars().count()) as i32)
            .sum::<i32>()
            + 2 * gap
    };
    if let Some(room) = room
        && width(gap, label) > room
    {
        gap = 2;
        if width(gap, label) > room {
            label = "answer";
        }
    }
    let mut xx = x;
    for (k, lab, on) in [
        ("g", "open pane", true),
        ("t", "transcript", true),
        ("a", label, can),
    ] {
        let start = xx;
        let segs = [
            Seg::new(k, if on { KEY } else { FAINT }).bold(),
            Seg::new(format!(" {lab}"), if on { FG } else { FAINT }),
        ];
        xx = s.segs(xx, y, &segs) + gap;
        if let (true, Some(r)) = (on, ident) {
            let t = match k {
                "g" => Target::Goto(r.clone()),
                "t" => Target::Transcript(r.clone()),
                _ => Target::Answer(r.clone()),
            };
            s.target(start, xx - gap, y, t);
        }
    }
}
