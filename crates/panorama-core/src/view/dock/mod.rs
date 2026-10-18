//! The docked panel (designs 3a and 9e, turn 10's focus look; Python `render.dock_panel`):
//! a header line, one entry per orchestrator the filter shows, `+ New agent`, and the
//! footer. Every entry is a click target for its orchestrator; its `?N` answers it.

mod chrome;
mod items;
mod look;
mod row;
mod state;
mod tiles;
mod tree_rows;

use super::filter::visible_orchs;
use super::orch_state::Ages;
use super::palette::{BG, DIM, DOCK_HOV, FAINT, HI, KEY};
use super::screen::{Screen, ScrollRegion};
use super::seg::Seg;
use super::state::{NEW_ROW, View};
use super::target::Target;
use super::{machines, scroll};
use crate::model::{Orchestrator, Swarm};
use row::{RowLook, dock_row};

/// The list's canvas: entries past this many rows are cut (Python `TALL`).
const TALL: i32 = 400;
/// The list's column, for scrolling.
const COLUMN: usize = 0;

/// The docked panel, `w` × `h`. `stale`: the last refresh failed. The list's scroll offset
/// is in the screen's [`ScrollRegion`]; the caller keeps it in `view.offsets`.
pub fn dock_panel(swarm: &Swarm, view: &View, w: i32, h: i32, stale: bool) -> Screen {
    let mut s = Screen::new(w, h);
    s.fill_bg(0, 0, w, h, BG);
    let orchs = visible_orchs(swarm, view);
    chrome::header(&mut s, swarm, orchs.len(), stale);
    chrome::footer(&mut s, view, swarm.inbox_ok);
    let (cv, sel, content_h) = list(swarm, view, &orchs, w);
    let (top, height) = (2, h - 4);
    let offset = scroll::offset(view, COLUMN, sel, height, content_h);
    s.blit(&cv, offset, 0, top, height);
    s.columns.push(ScrollRegion {
        x0: 0,
        x1: w,
        y0: top,
        y1: top + height,
        column: COLUMN,
        offset,
    });
    if let Some(look) = &view.panel {
        look::apply(&mut s, look);
    }
    s
}

/// The entries and `+ New agent` on a tall canvas; the selected one's rows and the height.
fn list(
    swarm: &Swarm,
    view: &View,
    orchs: &[&Orchestrator],
    w: i32,
) -> (Screen, Option<(i32, i32)>, i32) {
    let mut cv = Screen::new(w, TALL);
    let ages = Ages::of(view, swarm);
    let (mut y, mut sel) = (0, None);
    let mut group: Option<&str> = None;
    for o in orchs {
        if let Some(label) = machines::group_start(o, &mut group) {
            machines::divider(&mut cv, y, w, label);
            y += 1;
        }
        let cursor = view.orch.as_deref() == Some(o.pane_id.as_str());
        let docked = swarm.docked.as_deref() == Some(o.pane_id.as_str());
        let height = dock_row(&mut cv, y, o, view, ages, RowLook { docked, cursor }, w);
        if cursor {
            sel = Some((y, y + height));
        }
        y += height + 1;
    }
    let on_new = view.orch.as_deref() == Some(NEW_ROW);
    let hov = (on_new && view.cursor_look) || view.hover == Some(Target::NewAgent);
    cv.area(1, w - 1, y, y + 1, Target::NewAgent);
    if hov {
        cv.fill_bg(1, y, w - 2, 1, DOCK_HOV);
    }
    cv.segs(
        2,
        y,
        &[
            Seg::new("+", KEY).bold(),
            Seg::plain(" "),
            Seg::new("New agent", if hov { HI } else { DIM }),
        ],
    );
    cv.segs_right(w - 1, y, &[Seg::new("n", FAINT)]);
    if on_new {
        sel = Some((y, y + 1));
    }
    (cv, sel, y + 1)
}

#[cfg(test)]
mod tests;
