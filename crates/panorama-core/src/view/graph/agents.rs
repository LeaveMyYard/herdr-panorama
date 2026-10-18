//! The selected thread's agent tree in the wide graph (Python `_wide`'s last part): one canvas
//! for all depth columns, scrolled together, its cards joined by links within it, and to the
//! thread card on the left.

use super::agent_card::{AgentCtx, agent_card};
use super::column::Place;
use super::column::TALL;
use super::fin_card::fin_card;
use super::layout::Columns;
use super::place::{Tree, place_tree};
use crate::model::{Thread, TreeKids};
use crate::view::cards::details::agent_detail_lines;
use crate::view::cards::link::{Link, link};
use crate::view::nav::{TreeEntry, find_agent};
use crate::view::palette::FAINT;
use crate::view::screen::{Screen, ScrollRegion};
use crate::view::scroll;
use crate::view::seg::Seg;
use crate::view::state::View;

/// The agent columns' scroll column.
const COLUMN: usize = 2;

/// What the tree is drawn from: the thread, its entries, the columns, and the selected
/// agent's path.
pub struct TreeCtx<'a> {
    pub t: &'a Thread,
    pub entries: &'a [TreeEntry],
    pub cols: &'a Columns,
    pub path: &'a [String],
    pub view: &'a View,
    pub inbox_ok: bool,
}

impl TreeCtx<'_> {
    /// Whether depth `d`'s column is shown.
    fn vis(&self, d: usize) -> bool {
        self.cols.shown(1 + d)
    }

    /// An entry's height: the selected agent unfolds in a shown column.
    fn card_h(&self, e: &TreeEntry) -> i32 {
        let agent = find_agent(Some(self.t), Some(&e.ident));
        match agent {
            Some(a)
                if self.view.agent.as_deref() == Some(e.ident.as_str()) && self.vis(e.depth) =>
            {
                let w = self.cols.w(1 + e.depth) - 6;
                6 + agent_detail_lines(a, w, self.view.now, self.view.frame).len() as i32
            }
            _ => 3,
        }
    }
}

/// Draws the tree on `s` between rows `top` and `bottom`; `t_y`: the selected thread card's
/// link row on `s` (`None`: not shown).
pub fn agent_tree(s: &mut Screen, c: &TreeCtx, top: i32, bottom: i32, t_y: Option<i32>) {
    let height = bottom - top;
    let tree = place_tree(c.entries, |e| c.card_h(e));
    let mut cv = Screen::new(s.w(), TALL);
    let sel = cards(&mut cv, c, &tree);
    links(&mut cv, c, &tree);
    let off = scroll::offset(c.view, COLUMN, sel, height, tree.end);
    s.blit(&cv, off, 0, top, height);
    for d in (1..c.cols.len() - 1).filter(|d| c.vis(*d)) {
        let x0 = c.cols.x(1 + d);
        s.columns.push(ScrollRegion {
            x0,
            x1: x0 + c.cols.w(1 + d),
            y0: top,
            y1: bottom,
            column: COLUMN,
            offset: off,
        });
    }
    if c.vis(1) {
        let d1 = tree.groups.iter().find(|g| g.parent.is_none());
        let ys: Vec<i32> = d1.map_or(vec![], |g| {
            g.anchors.iter().map(|a| a - off + top).collect()
        });
        let fx = c.cols.x(1) + c.cols.w(1) - 1;
        let to_x = c.cols.x(2);
        let l = Link {
            fx,
            fy: t_y,
            tx: fx + 4,
            to_x,
            ys: &ys,
            top,
            bottom,
        };
        link(s, l);
    }
    if tree.end - off > height
        && let Some(first) = (1..c.cols.len() - 1).find(|d| c.vis(*d))
    {
        let more = [Seg::new("↓ ", FAINT), Seg::new("more below", FAINT)];
        s.segs(c.cols.x(1 + first) + 2, bottom, &more);
    }
}

/// The cards in shown columns; the selected card's rows.
fn cards(cv: &mut Screen, c: &TreeCtx, tree: &Tree) -> Option<(i32, i32)> {
    let kids = TreeKids::of(c.t);
    let on_path = &c.path[..c.path.len().saturating_sub(1)];
    let mut sel = None;
    for p in &tree.cards {
        let e = &c.entries[p.entry];
        if !c.vis(e.depth) || p.y + p.h > TALL {
            continue;
        }
        let col = 1 + e.depth;
        let is_sel = c.view.agent.as_deref() == Some(e.ident.as_str());
        let place = Place {
            x: c.cols.x(col),
            y: p.y,
            w: c.cols.w(col),
            is_sel,
            focus: is_sel && c.view.focus == col,
        };
        match find_agent(Some(c.t), Some(&e.ident)).filter(|_| e.fin.is_empty()) {
            None => fin_card(cv, place, e, c.t, c.view),
            Some(a) => {
                let n = kids.children(Some(&a.id)).len();
                let ctx = AgentCtx {
                    t: c.t,
                    inbox_ok: c.inbox_ok,
                    on_path: on_path.contains(&a.id),
                    kids_off: if c.vis(e.depth + 1) { 0 } else { n },
                };
                agent_card(cv, place, a, ctx, c.view);
            }
        }
        if is_sel {
            sel = Some((p.y, p.y + p.h));
        }
    }
    sel
}

/// Each shown group joined to the card that spawned it (or in from the left edge when that
/// column is off-screen, or from the scrolled-off thread column).
fn links(cv: &mut Screen, c: &TreeCtx, tree: &Tree) {
    for g in tree.groups.iter().filter(|g| c.vis(g.depth)) {
        let cx = c.cols.x(1 + g.depth);
        let l = |fx: i32, fy: i32, tx: i32| Link {
            fx,
            fy: Some(fy),
            tx,
            to_x: cx,
            ys: &g.anchors,
            top: 0,
            bottom: TALL,
        };
        match g.parent {
            Some(p) if c.vis(g.depth - 1) => {
                let fx = c.cols.x(g.depth) + c.cols.w(g.depth) - 1;
                link(cv, l(fx, p.y + 1, cx - 4));
            }
            Some(p) => link(cv, l(0, p.y + 1, 1)),
            None if !c.cols.shown(1) && !g.anchors.is_empty() => {
                link(cv, l((cx - 7).max(0), g.anchors[0], (cx - 4).max(1)));
            }
            None => {}
        }
    }
}
