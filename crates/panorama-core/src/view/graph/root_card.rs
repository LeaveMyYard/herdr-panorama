//! A root's card, the graph's depth-0 card (plan `uniform-agent-tree.md` §4): its pod, title
//! and the tree's need marks, its state words, and `tab N · age` on the bottom border. The
//! selected root, with the keyboard on it, unfolds with its own state: the prompt it blocks
//! on, the questions no agent filed, its background work, the finished agents the window
//! hid, and its action hints.

use super::card::{Line, Look, draw_frame, line, own_out};
use crate::model::{HerdrStatus, Orchestrator, Status};
use crate::view::cards::Area;
use crate::view::cards::actions::hint_row;
use crate::view::cards::details::bg_row;
use crate::view::cards::marks::marks_of;
use crate::view::cards::tags::tags;
use crate::view::orch_state::{Ages, OrchState, orch_state};
use crate::view::palette::{DIM, FAINT, FG, HI, status};
use crate::view::pod::pod;
use crate::view::screen::Screen;
use crate::view::seg::Seg;
use crate::view::state::View;
use crate::view::target::Target;
use crate::view::text::{dur, trunc, wrap};
use crate::view::unseen::mark_of;
use std::collections::HashSet;

/// How a root card sits: its row, width, whether it is the selected root and whether the
/// keyboard is on it, whether the inbox answers.
#[derive(Clone, Copy, Debug)]
pub struct RootPlace {
    pub y: i32,
    pub w: i32,
    pub is_sel: bool,
    pub focus: bool,
    pub inbox_ok: bool,
}

/// The questions in `o`'s inbox that no agent filed, with their texts when known.
pub fn loose_questions(o: &Orchestrator) -> Vec<&str> {
    let filed: HashSet<i64> = o
        .agents()
        .flat_map(|a| a.question_ids.iter().copied())
        .collect();
    o.inbox
        .iter()
        .enumerate()
        .filter(|(_, q)| !filed.contains(q))
        .map(|(i, _)| o.inbox_texts.get(i).map_or("", String::as_str))
        .collect()
}

/// The unfolded card's detail lines in `w` cells.
pub fn root_detail_lines(o: &Orchestrator, w: i32, view: &View) -> Vec<Vec<Seg>> {
    let mut out = vec![];
    let led = |text: &str, lead: &str, st: &Status, out: &mut Vec<Vec<Seg>>| {
        let col = status(st);
        for (i, l) in wrap(text, w - 2).into_iter().take(4).enumerate() {
            out.push(vec![
                Seg::new(if i > 0 { "  " } else { lead }, col).bold(),
                Seg::new(l, col),
            ]);
        }
    };
    for p in &o.pending {
        led(p, "! ", &Status::Blocked, &mut out);
    }
    for q in loose_questions(o) {
        let q = if q.is_empty() { "a question" } else { q };
        led(q, "? ", &Status::Waiting, &mut out);
    }
    for (i, b) in o.bg.iter().enumerate() {
        out.push(bg_row(b, w - 1, view.now, view.frame, i));
    }
    // the root's own fold says it (`+N older`) when there is one
    let folded = || {
        crate::view::nav::tree_entries(o, view)
            .iter()
            .any(|e| !e.fin.is_empty() && e.key == o.pane_id)
    };
    if o.hidden_finished > 0 && !folded() {
        let text = format!("+{} older finished, hidden", o.hidden_finished);
        out.push(vec![Seg::new(trunc(&text, w), FAINT)]);
    }
    out
}

/// `o`'s card at `p`. Its height.
pub fn root_card(cv: &mut Screen, p: RootPlace, o: &Orchestrator, view: &View, ages: Ages) -> i32 {
    let RootPlace { y, w, is_sel, .. } = p;
    let st = orch_state(o, view.frame, Some(ages));
    let open = is_sel && p.focus;
    let lines = if open {
        root_detail_lines(o, w - 6, view)
    } else {
        vec![]
    };
    let hh = match (open, lines.len() as i32) {
        (false, _) => 4,
        (true, 0) => 6,
        (true, n) => 7 + n,
    };
    let card = Area { x: 0, y, w, h: hh };
    let look = Look {
        target: Target::Orch(o.pane_id.clone()),
        focus: p.focus,
        is_sel,
        st: &st,
        seed: 0,
        pulse: mark_of(o.agents(), &view.unseen, true),
    };
    draw_frame(cv, card, &look, view);
    pod(cv, 2, y + 1, &st.st, &st.glyph, false);
    let agents: Vec<_> = o.agents().collect();
    let blocked = o.herdr_status == HerdrStatus::Blocked;
    let mut marks = marks_of(&agents, blocked, loose_questions(o).len());
    own_out(&mut marks, &st.st);
    let name_fg = if is_sel {
        HI
    } else if st.st == Status::Stale {
        FAINT
    } else if st.quiet() {
        DIM
    } else {
        FG
    };
    let title = crate::view::harness::title(o, view.harness_icons);
    let l = Line {
        title: vec![Seg::new(title, name_fg).bold()],
        doing: None,
        marks,
        orch: Some(&o.pane_id),
        right: vec![],
    };
    line(cv, 0, y + 1, w, l);
    cv.segs(6, y + 2, &state_line(o, &st, w));
    for (i, l) in lines.iter().enumerate() {
        cv.segs(4, y + 4 + i as i32, l);
    }
    if open {
        hints(cv, (4, y + hh - 2), w - 6, o, p.inbox_ok);
    }
    let tab = o.tab_no.map(|n| format!("tab {n}"));
    let age = o
        .last_activity
        .filter(|t| *t != 0.0)
        .map(|t| dur(Some(view.now - t)));
    tags(cv, 0, y + hh - 1, w, &[tab, age]);
    hh
}

/// The state words, then ` · live/agents` (or ` · n bg` while it waits on background work).
fn state_line(o: &Orchestrator, st: &OrchState, w: i32) -> [Seg; 2] {
    let label = trunc(&st.label, w - 8);
    let room = (w - 8 - label.chars().count() as i32).max(0);
    let rest = if st.st == Status::Background {
        trunc(&format!(" · {} bg", st.bg), room)
    } else if st.quiet() {
        String::new()
    } else {
        let live = o.agents().filter(|a| a.status.is_live()).count();
        trunc(&format!(" · {live}/{}", o.agents().count()), room)
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

/// The root's `g open pane`, `t transcript` and `a answer` (blocked: in its pane) from `at`,
/// in `room` cells, each enabled one a click target.
fn hints(cv: &mut Screen, at: (i32, i32), room: i32, o: &Orchestrator, inbox_ok: bool) {
    let blocked = o.herdr_status == HerdrStatus::Blocked;
    let can_answer = blocked || (inbox_ok && !o.inbox.is_empty());
    let pane = &o.pane_id;
    let hints = [
        (true, Some(Target::GotoOrch(pane.clone()))),
        (true, Some(Target::TranscriptOrch(pane.clone()))),
        (can_answer, Some(Target::AnswerOrch(pane.clone()))),
    ];
    let answer = if blocked { "answer in pane" } else { "answer" };
    hint_row(cv, at, hints, answer, Some(room));
}
