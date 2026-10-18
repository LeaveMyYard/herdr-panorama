//! The narrow layout (Python `render._narrow`, below 120 columns): one column of
//! orchestrators; the selected one opens its thread cards below it, the selected thread its
//! phases and its selected agent's details, joined to the orchestrator by a tree of lines.
//! A per-tab panel shows it when its tab has no orchestrator (nothing to dock next to).

mod card;
mod links;
mod rows;

use super::filter::visible_orchs;
use super::orch_state::Ages;
use super::palette::{BG, DIM, FAINT, HI, KEY};
use super::screen::{Screen, ScrollRegion};
use super::seg::Seg;
use super::state::{NEW_ROW, View};
use super::target::Target;
use super::{chrome, machines, scroll};
use crate::model::Swarm;

/// The list's canvas: rows past this are cut (Python `TALL`).
const TALL: i32 = 400;
/// The list's column, for scrolling.
const COLUMN: usize = 0;

/// The narrow page, `w` × `h`: the graph's header, the list, the legend and the keys.
/// `stale`: why the model may be out of date.
pub fn page(swarm: &Swarm, view: &View, w: i32, h: i32, stale: Option<&str>) -> Screen {
    let mut s = Screen::new(w, h);
    s.fill_bg(0, 0, w, h, BG);
    chrome::header(&mut s, swarm, view, stale);
    list(&mut s, swarm, view);
    // the narrow layout's chrome at any width (Python draws the wide graph from 120 columns,
    // P4; until then a wider pane shows this page whole, not wide chrome around it)
    chrome::legend(&mut s, h - 3, false);
    chrome::keys(&mut s, false, false, view.zoom_hint, swarm.inbox_ok);
    s
}

/// The orchestrators (the selected one opened), `+ New agent`, scrolled to the selection.
fn list(s: &mut Screen, swarm: &Swarm, view: &View) {
    let (w, h) = (s.w(), s.h());
    let mut cv = Screen::new(w, TALL);
    let ages = Ages::of(view, swarm);
    let (mut y, mut sel) = (0, None);
    let mut group: Option<&str> = None;
    for o in visible_orchs(swarm, view) {
        if let Some(label) = machines::group_start(o, &mut group) {
            machines::divider(&mut cv, y, w, label);
            y += 1;
        }
        if view.orch.as_deref() != Some(o.pane_id.as_str()) {
            rows::orch_row(&mut cv, y, o, view, ages);
            y += 2;
            continue;
        }
        let opened = rows::open_orch(&mut cv, y, o, swarm, view, ages);
        if let Some(rows) = opened.sel {
            sel = Some(rows);
        }
        y = opened.end + 1;
    }
    let is_new = view.orch.as_deref() == Some(NEW_ROW);
    cv.area(0, w, y, y + 1, Target::NewAgent);
    if !is_new && view.hover == Some(Target::NewAgent) {
        cv.put(0, y, "▏", super::cards::boxes::HOVER, None, false);
    }
    let segs = [
        Seg::new(" + ", KEY).bold(),
        Seg::new("  New agent", if is_new { HI } else { DIM }).bold_if(is_new),
        Seg::new("  n", FAINT),
    ];
    cv.segs(1, y, &segs);
    if is_new {
        cv.put(
            0,
            y,
            "▌",
            if view.focus == 0 { HI } else { DIM },
            None,
            false,
        );
        sel = Some((y, y + 1));
    }
    y += 1;
    let (top, height) = (2, h - 2 - 3);
    let offset = scroll::offset(view, COLUMN, sel, height, y);
    s.blit(&cv, offset, 0, top, height);
    s.columns.push(ScrollRegion {
        x0: 0,
        x1: w,
        y0: top,
        y1: top + height,
        column: COLUMN,
        offset,
    });
}

#[cfg(test)]
mod tests;
