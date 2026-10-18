//! The wide graph's columns (Python `render._wide`): orchestrators | threads | depth 1 |
//! depth 2 | … (as deep as the selected thread's tree), scrolled sideways around the focus,
//! each joined to the next by links.

use super::agents::{TreeCtx, agent_tree};
use super::column::Column;
use super::layout::{Columns, tier};
use super::orchs::orch_column;
use super::sideways;
use super::threads::thread_column;
use crate::model::Swarm;
use crate::view::cards::link::{Link, link};
use crate::view::filter::visible_orchs;
use crate::view::nav::{find_orch, find_thread, path_ids, tree_entries};
use crate::view::screen::{Screen, ScrollRegion};
use crate::view::scroll;
use crate::view::state::View;

/// Draws the columns on `s` (rows 2 to `h - 4`). Whether some are off-screen.
pub fn columns(s: &mut Screen, swarm: &Swarm, view: &View) -> bool {
    let (w, h) = (s.w(), s.h());
    let (ow, tw, dw) = tier(w);
    let (top, bottom) = (3, h - 4);
    let o = find_orch(swarm, view.orch.as_deref()).filter(|o| {
        visible_orchs(swarm, view)
            .iter()
            .any(|v| v.pane_id == o.pane_id)
    });
    let t = find_thread(o, view.thread.as_deref());
    let entries = t.map_or(vec![], |t| tree_entries(t, view));
    let depth = entries.iter().map(|e| e.depth).max().unwrap_or(1);
    let widths = match (o, t) {
        (None, _) => vec![ow],
        (Some(_), None) => vec![ow, tw, dw], // 2a's empty agent column
        (Some(_), Some(_)) => [vec![ow, tw], vec![dw; depth]].concat(),
    };
    let cols = Columns::place(widths, w, view.focus, view.hcol);
    s.hlo = cols.lo;
    let path = path_ids(t, view.agent.as_deref());
    sideways::line(s, 2, &cols, o, t, &path);
    let shown = |c: Column, i: usize, s: &mut Screen| {
        let x0 = cols.x(i);
        let off = scroll::offset(view, i, c.sel, bottom - top, c.height);
        s.blit(&c.canvas, off, x0, top, bottom - top);
        let region = ScrollRegion {
            x0,
            x1: x0 + cols.w(i),
            y0: top,
            y1: bottom,
            column: i,
            offset: off,
        };
        s.columns.push(region);
        (
            c.anchors.iter().map(|a| a - off + top).collect::<Vec<_>>(),
            c.sel_anchor.map(|a| a - off + top),
        )
    };
    let orch_y = if cols.shown(0) {
        shown(orch_column(swarm, view, ow), 0, s).1
    } else {
        None
    };
    let Some(o) = o else { return cols.sideways() };
    let mut t_y = None;
    if cols.shown(1) {
        let (anchors, sel) = shown(thread_column(o, view, cols.w(1)), 1, s);
        let to_x = cols.x(1);
        let l = if cols.shown(0) {
            let fx = cols.x(0) + ow - 1;
            Link {
                fx,
                fy: orch_y,
                tx: fx + 3,
                to_x,
                ys: &anchors,
                top,
                bottom,
            }
        } else {
            Link {
                fx: 0,
                fy: anchors.first().copied(),
                tx: 1,
                to_x,
                ys: &anchors,
                top,
                bottom,
            }
        };
        link(s, l);
        t_y = sel;
    }
    let Some(t) = t else { return cols.sideways() };
    let ctx = TreeCtx {
        t,
        entries: &entries,
        cols: &cols,
        path: &path,
        view,
        inbox_ok: swarm.inbox_ok,
    };
    agent_tree(s, &ctx, top, bottom, t_y);
    cols.sideways()
}
