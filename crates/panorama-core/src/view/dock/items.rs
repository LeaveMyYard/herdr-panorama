//! The docked entry's list (plan `uniform-agent-tree.md` §4, V11 sidebar): the root's
//! background tasks, then one row per agent still going in two layers (the root's direct
//! subagents, then theirs), then one row folding the root's finished subagents. Every agent
//! row is the same: its own glyph, its title, and a run of states on the right. Nothing
//! below layer 2 gets a row.

use super::state::DockState;
use super::tiles::{agent_glyphs, glyph_colour, tile};
use crate::model::{Agent, BgTask, Root, Status};
use crate::view::agent_text::name;
use crate::view::anim::{clock, glyph, pulse};
use crate::view::palette::{BG, DIM, FAINT, FG, LINE};
use crate::view::screen::Screen;
use crate::view::seg::{Seg, width};
use crate::view::state::View;
use crate::view::text::{dur, trunc};
use crate::view::unseen::mark_of;

/// One row of the list.
pub enum Item<'a> {
    /// A background task of the root itself.
    Bg(&'a BgTask),
    /// A direct subagent of the root, and its finished children (its run: `+N✓`; its
    /// children still going get their own rows under it).
    Agent(&'a Agent, Vec<&'a Agent>),
    /// A layer-2 agent, whether it is its parent's last such row, and all its descendants
    /// (its run).
    Sub(&'a Agent, bool, Vec<&'a Agent>),
    /// The root's finished subagents, folded.
    Fin,
}

impl Item<'_> {
    /// Whether the row hangs off the entry's own branch (everything but a layer-2 row).
    pub fn is_head(&self) -> bool {
        !matches!(self, Item::Sub(..))
    }
}

/// The rows of the docked entry, in order.
pub fn items_of<'a>(o: &'a Root, d: &DockState<'a>) -> Vec<Item<'a>> {
    let mut out: Vec<Item> = o.bg.iter().map(Item::Bg).collect();
    for a in &d.active {
        let (going, fin) = d.tree.split(Some(&a.id));
        out.push(Item::Agent(a, fin));
        let n = going.len();
        for (i, k) in going.into_iter().enumerate() {
            out.push(Item::Sub(k, i + 1 == n, d.tree.descendants(k)));
        }
    }
    if !d.done.is_empty() || o.hidden_finished > 0 {
        out.push(Item::Fin);
    }
    out
}

/// What every row of one entry's list shares.
pub struct ItemCtx<'a> {
    pub o: &'a Root,
    pub view: &'a View,
    pub d: &'a DockState<'a>,
    /// Where right-aligned fields end.
    pub xr: i32,
    /// The index of the last row on the entry's own branch.
    pub last_head: Option<usize>,
}

/// Draws list row `i` at `yy`.
pub fn draw_item(cv: &mut Screen, yy: i32, item: &Item, i: usize, ctx: &ItemCtx) {
    let (view, xr) = (ctx.view, ctx.xr);
    if let Item::Sub(a, last, below) = item {
        if ctx.last_head.is_some_and(|l| i < l) {
            cv.put(3, yy, "│", LINE, None, false);
        }
        let branch = if *last { "└" } else { "├" };
        let x = cv.segs(5, yy, &[Seg::new(branch, LINE)]) + 1;
        agent_row(cv, (x, yy), a, below, 6, ctx);
        return;
    }
    let branch = if Some(i) == ctx.last_head {
        "└"
    } else {
        "├"
    };
    cv.put(3, yy, branch, LINE, None, false);
    match item {
        Item::Bg(t) => draw_bg(cv, yy, t, i, view, xr),
        Item::Fin => draw_fin(cv, yy, ctx),
        Item::Agent(a, fin) => agent_row(cv, (5, yy), a, fin, 0, ctx),
        Item::Sub(..) => {}
    }
}

/// The one agent row: its own glyph at `x`, its title, and on the right the run of
/// `below` (`n` live glyphs at most). An unseen mark on the agent or in its run pulses the
/// glyph's background, as a tile's does.
fn agent_row(
    cv: &mut Screen,
    (x, yy): (i32, i32),
    a: &Agent,
    below: &[&Agent],
    n: usize,
    ctx: &ItemCtx,
) {
    let (view, xr) = (ctx.view, ctx.xr);
    let run = agent_glyphs(below, view.frame, n);
    let g = glyph(a, view.frame);
    let room = (xr - width(&run) - 1 - (x + g.chars().count() as i32 + 1)).max(1);
    let att = a.status.needs_operator();
    let m = mark_of(
        std::iter::once(a).chain(below.iter().copied()),
        &view.unseen,
        true,
    );
    let mut glyph_seg = Seg::new(g, glyph_colour(&a.status, view.frame))
        .bold_if(att)
        .fg_moving(att);
    if let Some(m) = m {
        glyph_seg = glyph_seg.on(pulse(view.frame, m, BG, 1.0)).bg_moving(true);
    }
    let title = Seg::new(trunc(&name(a), room), FG);
    cv.segs(x, yy, &[glyph_seg, Seg::plain(" "), title]);
    cv.segs_right(xr, yy, &run);
}

/// A background task: its clock tile, what it does, and its kind and age on the right.
fn draw_bg(cv: &mut Screen, yy: i32, t: &BgTask, i: usize, view: &View, xr: i32) {
    let frame = view.frame;
    let kind = t.kind.to_lowercase();
    let when = match t.started {
        // Python's truthiness: a start time of 0 counts as none
        Some(s) if s != 0.0 => format!("{kind} {}", dur(Some(view.now - s))),
        _ => kind,
    };
    let tl = tile(
        &Status::Background,
        &clock(frame, i as i64 + 1).to_string(),
        frame,
        None,
    );
    let room = (xr - when.chars().count() as i32 - 1 - (5 + tl.len() + 1)).clamp(1, 22);
    let desc = if t.description.is_empty() {
        "in the background"
    } else {
        t.description.as_str()
    };
    cv.segs(
        5,
        yy,
        &[tl, Seg::plain(" "), Seg::new(trunc(desc, room), FG)],
    );
    cv.segs_right(xr, yy, &[Seg::new(when, DIM)]);
}

/// The root's finished subagents in one row, `✓ N finished · names`, and `+N older` for
/// those the done window dropped; unseen, its tile pulses.
fn draw_fin(cv: &mut Screen, yy: i32, ctx: &ItemCtx) {
    let (d, view) = (ctx.d, ctx.view);
    let below = d.done.iter().flat_map(|a| d.tree.subtree(a));
    let fm = mark_of(below, &view.unseen, true);
    let older = ctx.o.hidden_finished;
    let right = if older > 0 {
        vec![Seg::new(format!("+{older} older"), FAINT)]
    } else {
        vec![]
    };
    let tl = tile(&Status::Done, "✓", view.frame, fm);
    let x = 5 + tl.len() + 1;
    let names: Vec<String> = d.done.iter().map(|a| name(a)).collect();
    let text = match names.len() {
        0 => String::new(),
        n => format!("{n} finished · {}", names.join(", ")),
    };
    let room = (ctx.xr - width(&right) - 1 - x).max(1);
    cv.segs(
        5,
        yy,
        &[tl, Seg::plain(" "), Seg::new(trunc(&text, room), FAINT)],
    );
    cv.segs_right(ctx.xr, yy, &right);
}
