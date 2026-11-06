//! The wide graph's depth-0 column (Python `render.orch_column`): a card per root the filter
//! shows (grouped under a divider per remote machine), then `+ New agent`.

use super::column::Column;
use super::root_card::{RootPlace, root_card};
use crate::model::Swarm;
use crate::view::cards::boxes::{border, rbox};
use crate::view::filter::visible_orchs;
use crate::view::machines;
use crate::view::orch_state::Ages;
use crate::view::palette::{DIM, FAINT, HI, KEY};
use crate::view::seg::Seg;
use crate::view::state::{NEW_ROW, View};
use crate::view::target::Target;

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
        let p = RootPlace {
            y,
            w,
            is_sel,
            focus: is_sel && view.focus == 0,
            inbox_ok: swarm.inbox_ok,
        };
        let h = root_card(&mut col.canvas, p, o, view, ages);
        col.card(y, h, is_sel);
        y += h + 1;
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
