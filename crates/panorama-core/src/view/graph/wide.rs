//! The wide graph's columns (Python `render._wide`, without the thread column since the
//! uniform agent tree): depth 0 (the roots) | depth 1 | depth 2 | … (as deep as the selected
//! root's tree), scrolled sideways around the focus, each joined to the next by links.

use super::agents::{TreeCtx, agent_tree};
use super::column::Column;
use super::layout::{Columns, tier};
use super::orchs::orch_column;
use super::sideways;
use crate::model::Swarm;
use crate::view::filter::visible_orchs;
use crate::view::nav::{find_orch, path_ids, tree_entries};
use crate::view::palette::FAINT;
use crate::view::screen::{Screen, ScrollRegion};
use crate::view::scroll;
use crate::view::seg::Seg;
use crate::view::state::View;

/// Draws the columns on `s` (rows 2 to `h - 4`). Whether some are off-screen.
pub fn columns(s: &mut Screen, swarm: &Swarm, view: &View) -> bool {
    let (w, h) = (s.w(), s.h());
    let (rw, dw) = tier(w);
    let (top, bottom) = (3, h - 4);
    let o = find_orch(swarm, view.orch.as_deref()).filter(|o| {
        visible_orchs(swarm, view)
            .iter()
            .any(|v| v.pane_id == o.pane_id)
    });
    let entries = o.map_or(vec![], |o| tree_entries(o, view));
    let depth = entries.iter().map(|e| e.depth).max().unwrap_or(1);
    let widths = match o {
        None => vec![rw],
        Some(_) => [vec![rw], vec![dw; depth]].concat(),
    };
    let cols = Columns::place(widths, w, view.focus, view.hcol);
    s.hlo = cols.lo;
    let path = path_ids(o, view.agent.as_deref());
    sideways::line(s, 2, &cols, o, &path);
    let root_y = if cols.shown(0) {
        let c: Column = orch_column(swarm, view, rw);
        let off = scroll::offset(view, 0, c.sel, bottom - top, c.height);
        s.blit(&c.canvas, off, cols.x(0), top, bottom - top);
        s.columns.push(ScrollRegion {
            x0: cols.x(0),
            x1: cols.x(0) + rw,
            y0: top,
            y1: bottom,
            column: 0,
            offset: off,
        });
        c.sel_anchor.map(|a| a - off + top)
    } else {
        None
    };
    let Some(o) = o else { return cols.sideways() };
    if entries.is_empty() && cols.shown(1) {
        let none = Seg::new("no subagents in the last hours", FAINT);
        s.segs(cols.x(1) + 2, top, &[none]);
    }
    let ctx = TreeCtx {
        o,
        entries: &entries,
        cols: &cols,
        path: &path,
        view,
        inbox_ok: swarm.inbox_ok,
    };
    agent_tree(s, &ctx, top, bottom, root_y);
    cols.sideways()
}
