//! A card's marks on the right: `✗` (an agent failed), `!` (blocked), `?N` (questions)
//! (Python `render.marks_of`, `marks_right`).

use crate::model::{Agent, Status};
use crate::view::palette::status;
use crate::view::screen::Screen;
use crate::view::seg::Seg;
use crate::view::target::Target;

/// The marks of `agents` (`blocked`: the session itself is blocked).
pub fn marks_of(agents: &[&Agent], blocked: bool) -> Vec<Seg> {
    let any = |st: Status| agents.iter().any(|a| a.status == st);
    let mut out = vec![];
    if any(Status::Failed) {
        out.push(Seg::new("✗", status(&Status::Failed)).bold());
    }
    if blocked || any(Status::Blocked) {
        out.push(Seg::new("!", status(&Status::Blocked)).bold());
    }
    let q: usize = agents.iter().map(|a| a.question_ids.len()).sum();
    if q > 0 {
        out.push(Seg::new(format!("?{q}"), status(&Status::Waiting)).bold());
    }
    out
}

/// The marks right-aligned at `xr`, one space apart; with `orch`, the `?N` mark is a click
/// target answering that orchestrator. Their width (0 when none).
pub fn marks_right(s: &mut Screen, xr: i32, y: i32, mk: &[Seg], orch: Option<&str>) -> i32 {
    let w = mk.iter().map(|m| m.len() + 1).sum::<i32>() - 1;
    let mut x = xr - w;
    for m in mk {
        let start = x;
        x = s.put_seg(x, y, m) + 1;
        if let Some(o) = orch
            && m.text.starts_with('?')
        {
            s.target(start, x - 1, y, Target::AnswerOrch(o.to_owned()));
        }
    }
    w.max(0)
}
