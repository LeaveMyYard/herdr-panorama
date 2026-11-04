//! The selected root's agent tree in the wide graph (Python `_wide`'s last part): one canvas
//! for all depth columns, scrolled together, its cards joined by links within it, and to the
//! root's card on the left.

use super::agent_card::{AgentCtx, agent_card, detail_lines};
use super::column::Place;
use super::column::TALL;
use super::fin_card::fin_card;
use super::layout::Columns;
use super::place::{Tree, place_tree};
use crate::model::{Orchestrator, TreeKids};
use crate::view::cards::link::{Link, link};
use crate::view::nav::{TreeEntry, find_agent};
use crate::view::palette::FAINT;
use crate::view::screen::{Screen, ScrollRegion};
use crate::view::scroll;
use crate::view::seg::Seg;
use crate::view::state::View;

/// The agent columns' scroll column.
const COLUMN: usize = 1;

/// What the tree is drawn from: the root, its entries, the columns, and the selected
/// agent's path.
pub struct TreeCtx<'a> {
    pub o: &'a Orchestrator,
    pub entries: &'a [TreeEntry],
    pub cols: &'a Columns,
    pub path: &'a [String],
    pub view: &'a View,
    pub inbox_ok: bool,
}

impl TreeCtx<'_> {
    /// Whether depth `d`'s column is shown.
    fn vis(&self, d: usize) -> bool {
        self.cols.shown(d)
    }

    /// Whether `e` is the selected agent's card, unfolded: the keyboard is in the tree (on a
    /// root, the root's own card is open instead, its keys act on it).
    fn open(&self, e: &TreeEntry) -> bool {
        self.view.focus >= 1 && self.view.agent.as_deref() == Some(e.ident.as_str())
    }

    /// An entry's height: the selected agent unfolds in a shown column.
    fn card_h(&self, e: &TreeEntry) -> i32 {
        let agent = find_agent(Some(self.o), Some(&e.ident));
        match agent {
            Some(a) if self.open(e) && self.vis(e.depth) => {
                let w = self.cols.w(e.depth) - 6;
                6 + detail_lines(a, w, self.view).len() as i32
            }
            _ => 3,
        }
    }
}

/// Draws the tree on `s` between rows `top` and `bottom`; `root_y`: the selected root card's
/// link row on `s` (`None`: not shown).
pub fn agent_tree(s: &mut Screen, c: &TreeCtx, top: i32, bottom: i32, root_y: Option<i32>) {
    let height = bottom - top;
    let tree = place_tree(c.entries, |e| c.card_h(e));
    let mut cv = Screen::new(s.w(), TALL);
    let sel = cards(&mut cv, c, &tree);
    links(&mut cv, c, &tree);
    let off = scroll::offset(c.view, COLUMN, sel, height, tree.end);
    s.blit(&cv, off, 0, top, height);
    for d in (1..c.cols.len()).filter(|d| c.vis(*d)) {
        let x0 = c.cols.x(d);
        s.columns.push(ScrollRegion {
            x0,
            x1: x0 + c.cols.w(d),
            y0: top,
            y1: bottom,
            column: COLUMN,
            offset: off,
        });
    }
    if c.vis(1) && c.cols.shown(0) {
        let d1 = tree.groups.iter().find(|g| g.parent.is_none());
        let ys: Vec<i32> = d1.map_or(vec![], |g| {
            g.anchors.iter().map(|a| a - off + top).collect()
        });
        let fx = c.cols.x(0) + c.cols.w(0) - 1;
        let to_x = c.cols.x(1);
        let l = Link {
            fx,
            fy: root_y,
            tx: fx + 4,
            to_x,
            ys: &ys,
            top,
            bottom,
        };
        link(s, l);
    }
    if tree.end - off > height
        && let Some(first) = (1..c.cols.len()).find(|d| c.vis(*d))
    {
        let more = [Seg::new("↓ ", FAINT), Seg::new("more below", FAINT)];
        s.segs(c.cols.x(first) + 2, bottom, &more);
    }
}

/// The cards in shown columns; the selected card's rows.
fn cards(cv: &mut Screen, c: &TreeCtx, tree: &Tree) -> Option<(i32, i32)> {
    let kids = TreeKids::of_root(c.o);
    let on_path = &c.path[..c.path.len().saturating_sub(1)];
    let mut sel = None;
    for p in &tree.cards {
        let e = &c.entries[p.entry];
        if !c.vis(e.depth) || p.y + p.h > TALL {
            continue;
        }
        let col = e.depth;
        let chosen = c.view.agent.as_deref() == Some(e.ident.as_str());
        // a fold never unfolds: it reads selected whenever it is chosen
        let is_sel = if e.fin.is_empty() { c.open(e) } else { chosen };
        let place = Place {
            x: c.cols.x(col),
            y: p.y,
            w: c.cols.w(col),
            is_sel,
            focus: is_sel && c.view.focus == col,
        };
        match find_agent(Some(c.o), Some(&e.ident)).filter(|_| e.fin.is_empty()) {
            None => fin_card(cv, place, e, c.o, c.view),
            Some(a) => {
                let n = kids.children(Some(&a.id)).len();
                let ctx = AgentCtx {
                    o: c.o,
                    kids: &kids,
                    inbox_ok: c.inbox_ok,
                    // with the keyboard on the root, the agent below it still reads as chosen
                    on_path: on_path.contains(&a.id) || chosen,
                    kids_off: if c.vis(e.depth + 1) { 0 } else { n },
                };
                agent_card(cv, place, a, ctx, c.view);
            }
        }
        if chosen {
            sel = Some((p.y, p.y + p.h));
        }
    }
    sel
}

/// Each shown group joined to the card that spawned it (or in from the left edge when that
/// column is off-screen, or from the scrolled-off roots' column).
fn links(cv: &mut Screen, c: &TreeCtx, tree: &Tree) {
    for g in tree.groups.iter().filter(|g| c.vis(g.depth)) {
        let cx = c.cols.x(g.depth);
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
                let fx = c.cols.x(g.depth - 1) + c.cols.w(g.depth - 1) - 1;
                link(cv, l(fx, p.y + 1, cx - 4));
            }
            Some(p) => link(cv, l(0, p.y + 1, 1)),
            None if !c.cols.shown(0) && !g.anchors.is_empty() => {
                link(cv, l((cx - 7).max(0), g.anchors[0], (cx - 4).max(1)));
            }
            None => {}
        }
    }
}
