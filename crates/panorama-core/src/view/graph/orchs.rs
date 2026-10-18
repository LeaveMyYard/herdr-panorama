//! The wide graph's first column (Python `render.orch_column`): a card per orchestrator the
//! filter shows (grouped under a divider per remote machine), then `+ New agent`.

use super::column::Column;
use crate::model::{HerdrStatus, Orchestrator, Status, Swarm, ThreadSource};
use crate::view::cards::Area;
use crate::view::cards::boxes::{border, rbox};
use crate::view::cards::edge::edge;
use crate::view::cards::marks::{marks_of, marks_right};
use crate::view::cards::pulse::pulse_box;
use crate::view::cards::tags::tags;
use crate::view::filter::visible_orchs;
use crate::view::machines;
use crate::view::orch_state::{Ages, OrchState, orch_state};
use crate::view::palette::{DIM, FAINT, FG, HI, KEY, status};
use crate::view::pod::pod;
use crate::view::seg::Seg;
use crate::view::state::{NEW_ROW, View};
use crate::view::target::Target;
use crate::view::text::trunc;
use crate::view::unseen::mark_of;
use std::collections::HashSet;

/// The column, `w` cells wide.
pub fn orch_column(swarm: &Swarm, view: &View, w: i32) -> Column {
    let mut col = Column::new(w);
    let ages = Ages::of(view, swarm);
    let mut y = 0;
    let mut group: Option<&str> = None;
    for o in visible_orchs(swarm, view) {
        if let Some(label) = machines::group_start(o, &mut group) {
            machines::divider(&mut col.canvas, y, w, label);
            y += 1;
        }
        let is_sel = view.orch.as_deref() == Some(o.pane_id.as_str());
        orch_card(&mut col, y, o, view, ages, is_sel);
        col.card(y, 4, is_sel);
        y += 5;
    }
    let is_new = view.orch.as_deref() == Some(NEW_ROW);
    let cv = &mut col.canvas;
    cv.area(0, w, y, y + 3, Target::NewAgent);
    let bc = border(view, &Target::NewAgent, is_new && view.focus == 0, is_new);
    rbox(cv, 0, y, w, 3, bc);
    let label = Seg::new("  New agent", if is_new { HI } else { DIM }).bold_if(is_new);
    cv.segs(2, y + 1, &[Seg::new("+", KEY).bold(), label]);
    cv.segs(w - 3, y + 1, &[Seg::new("n", FAINT)]);
    if is_new {
        col.sel = Some((y, y + 3));
    }
    col.height = y + 4;
    col
}

/// The questions in `o`'s inbox that no agent filed.
fn loose_questions(o: &Orchestrator) -> usize {
    let filed: HashSet<i64> = o
        .agents()
        .flat_map(|a| a.question_ids.iter().copied())
        .collect();
    o.inbox.iter().filter(|q| !filed.contains(q)).count()
}

/// `o`'s card at row `y`: its state pod, name and marks, its state words and counts, and
/// its flavour on the bottom border.
fn orch_card(col: &mut Column, y: i32, o: &Orchestrator, view: &View, ages: Ages, is_sel: bool) {
    let cv = &mut col.canvas;
    let w = cv.w();
    let card = Area { x: 0, y, w, h: 4 };
    let st = orch_state(o, view.frame, Some(ages));
    let target = Target::Orch(o.pane_id.clone());
    cv.area(0, w, y, y + 4, target.clone());
    let bc = border(view, &target, is_sel && view.focus == 0, is_sel);
    rbox(cv, 0, y, w, 4, bc);
    edge(cv, card, &st, view.frame, 0);
    pulse_box(
        cv,
        card,
        mark_of(o.agents(), &view.unseen, true),
        view.frame,
        bc,
    );
    pod(cv, 2, y + 1, &st.st, &st.glyph, false);
    let agents: Vec<_> = o.agents().collect();
    let blocked = o.herdr_status == HerdrStatus::Blocked;
    let mut mk = marks_of(&agents, blocked, loose_questions(o));
    // the pod already says it
    mk.retain(|m| {
        !(m.text == "✗" && st.st == Status::Failed || m.text == "!" && st.st == Status::Blocked)
    });
    let mw = marks_right(cv, w - 2, y + 1, &mk, Some(&o.pane_id));
    let stale = st.st == Status::Stale;
    let name_fg = if is_sel {
        HI
    } else if stale {
        FAINT
    } else if st.quiet() {
        DIM
    } else {
        FG
    };
    let name = trunc(&o.tab_label, w - 9 - mw);
    cv.segs(6, y + 1, &[Seg::new(name, name_fg).bold()]);
    cv.segs(6, y + 2, &state_line(o, &st, w));
    tags(cv, 0, y + 3, w, &[Some(o.flavour.clone())]);
}

/// The state words, then ` · active/live` (or ` · n bg` while it waits on background work).
fn state_line(o: &Orchestrator, st: &OrchState, w: i32) -> [Seg; 2] {
    let label = trunc(&st.label, w - 8);
    let room = (w - 8 - label.chars().count() as i32).max(0);
    let rest = if st.st == Status::Background {
        trunc(&format!(" · {} bg", st.bg), room)
    } else if st.quiet() {
        String::new()
    } else {
        let active = o
            .threads
            .iter()
            .filter(|t| !t.is_done() && t.source != ThreadSource::Session)
            .count();
        let live = o
            .agents()
            .filter(|a| a.status.is_live() && !a.id.starts_with("main:"))
            .count();
        trunc(&format!(" · {active}/{live}"), room)
    };
    let col = if st.st == Status::Stale {
        status(&Status::Stale)
    } else if st.quiet() {
        FAINT
    } else {
        status(&st.st)
    };
    [
        Seg::new(label, col).bold_if(!st.quiet()),
        Seg::new(rest, DIM),
    ]
}
