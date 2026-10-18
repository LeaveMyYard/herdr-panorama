//! The wide graph's second column (Python `render.thread_column`): the selected
//! orchestrator's background card, its active threads, and its done threads folded into one
//! "n complete" card (unfolded below it on request).

use super::column::Column;
use super::column::Place;
use super::thread_card::{bg_card, thread_card};
use crate::model::{Orchestrator, Status, Thread};
use crate::view::cards::Area;
use crate::view::cards::boxes::{border, rbox};
use crate::view::cards::pulse::pulse_box;
use crate::view::nav::split_threads;
use crate::view::palette::{FAINT, FG, status};
use crate::view::seg::Seg;
use crate::view::state::{BG_CARD, DONE_CARD, View};
use crate::view::target::Target;
use crate::view::text::trunc;
use crate::view::unseen::mark_of;

/// `o`'s column, `w` cells wide.
pub fn thread_column(o: &Orchestrator, view: &View, w: i32) -> Column {
    let mut col = Column::new(w);
    let (active, done) = split_threads(o, view.filter);
    let selected = |id: &str| view.thread.as_deref() == Some(id);
    let place = |y: i32, is_sel: bool| Place {
        x: 0,
        y,
        w,
        is_sel,
        focus: is_sel && view.focus == 1,
    };
    let mut y = 0;
    if !o.bg.is_empty() {
        let is_sel = selected(&format!("{BG_CARD}{}", o.pane_id));
        let hh = bg_card(&mut col.canvas, place(y, is_sel), o, view);
        col.card(y, hh, is_sel);
        y += hh + 1;
    }
    let threads = |col: &mut Column, y: &mut i32, ts: &[&Thread]| {
        for t in ts {
            let is_sel = selected(&t.id);
            let seed = col.anchors.len() as i64 + 1;
            let hh = thread_card(
                &mut col.canvas,
                place(*y, is_sel),
                t,
                view,
                seed,
                &o.pane_id,
            );
            col.card(*y, hh, is_sel);
            *y += hh + 1;
        }
    };
    threads(&mut col, &mut y, &active);
    if !done.is_empty() {
        let is_sel = selected(&format!("{DONE_CARD}{}", o.pane_id));
        done_card(&mut col, y, o, &done, view, is_sel);
        col.card(y, 3, is_sel);
        y += 4;
        if view.done_open.contains(&o.pane_id) {
            threads(&mut col, &mut y, &done);
        }
    }
    if active.is_empty() && done.is_empty() && o.bg.is_empty() {
        let none = Seg::new("no subagents in the last hours", FAINT);
        col.canvas.segs(2, 0, &[none]);
        y = 1;
    }
    col.height = y;
    col
}

/// The done threads folded into one card at row `y`: `✓ n complete  names`, and whether it
/// is unfolded.
fn done_card(
    col: &mut Column,
    y: i32,
    o: &Orchestrator,
    done: &[&Thread],
    view: &View,
    is_sel: bool,
) {
    let cv = &mut col.canvas;
    let w = cv.w();
    let target = Target::Done(o.pane_id.clone());
    cv.area(0, w, y, y + 3, target.clone());
    let bc = border(view, &target, is_sel && view.focus == 1, is_sel);
    rbox(cv, 0, y, w, 3, bc);
    let agents = done.iter().flat_map(|t| t.agents.iter());
    let card = Area { x: 0, y, w, h: 3 };
    pulse_box(
        cv,
        card,
        mark_of(agents, &view.unseen, true),
        view.frame,
        bc,
    );
    let names: Vec<&str> = done.iter().map(|t| t.name.as_str()).collect();
    let segs = [
        Seg::new("✓", status(&Status::Done)).bold(),
        Seg::new(format!(" {} complete", done.len()), FG),
        Seg::new(format!("  {}", trunc(&names.join(", "), w - 21)), FAINT),
    ];
    cv.segs(2, y + 1, &segs);
    let fold = if view.done_open.contains(&o.pane_id) {
        "▾"
    } else {
        "▸"
    };
    cv.put(w - 3, y + 1, fold, FAINT, None, false);
}
