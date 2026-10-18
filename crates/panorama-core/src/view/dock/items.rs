//! The docked entry's list (9e; Python `render.dock_row`'s items): the session's background
//! tasks, then each active thread with the tree rows of its spawning agents, then one row for
//! the done threads. Each row starts with a tile, as the thread rows do.

use super::state::{DockState, thread_glyph};
use super::tiles::{agent_glyphs, glyph_colour, tile};
use super::tree_rows::{SpawnRow, spawn_rows};
use crate::model::{Agent, BgTask, Orchestrator, Status, Thread, TreeKids};
use crate::view::anim::{clock, glyph};
use crate::view::palette::{DIM, FAINT, FG, LINE};
use crate::view::screen::Screen;
use crate::view::seg::{Seg, width};
use crate::view::state::View;
use crate::view::text::{dur, trunc};
use crate::view::unseen::mark_of;
use herdr_agent_kit::herdr_api::{MAX_ARG, clean};

/// One row of the list.
pub enum Item<'a> {
    /// A background task of the session.
    Bg(&'a BgTask),
    /// An active thread, and the agents it spawned itself.
    Thread(&'a Thread, Vec<&'a Agent>),
    /// An agent of a thread that spawned others, and those it spawned.
    Spawn(SpawnRow<'a>, Vec<&'a Agent>),
    /// The done threads, folded.
    Done,
}

impl Item<'_> {
    /// Whether the row hangs off the entry's own branch (everything but a tree row).
    pub fn is_head(&self) -> bool {
        !matches!(self, Item::Spawn(..))
    }
}

/// The rows of the docked entry, in order.
pub fn items_of<'a>(o: &'a Orchestrator, d: &DockState<'a>) -> Vec<Item<'a>> {
    let mut out: Vec<Item> = o.bg.iter().map(Item::Bg).collect();
    for t in &d.active {
        let kids = TreeKids::of(t);
        out.push(Item::Thread(t, kids.children(None).to_vec()));
        for row in spawn_rows(&kids) {
            let below = kids.children(Some(&row.agent.id)).to_vec();
            out.push(Item::Spawn(row, below));
        }
    }
    if !d.done.is_empty() {
        out.push(Item::Done);
    }
    out
}

fn cleaned(s: &str) -> String {
    clean(s, MAX_ARG)
}

/// Python `role_label`: the role, else the agent type, else "agent".
fn role_label(a: &Agent) -> &str {
    [a.role.as_str(), a.agent_type.as_deref().unwrap_or("")]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or("agent")
}

/// Python `short_id`: "main" for the session's own work, else the first 7 characters.
fn short_id(a: &Agent) -> String {
    if a.id.starts_with("main:") {
        "main".into()
    } else {
        a.id.chars().take(7).collect()
    }
}

/// Draws list row `i` at `yy`; `last_thread` is the index of the last head row.
#[allow(clippy::too_many_arguments)]
pub fn draw_item(
    cv: &mut Screen,
    yy: i32,
    item: &Item,
    i: usize,
    last_thread: Option<usize>,
    d: &DockState,
    view: &View,
    xr: i32,
) {
    match item {
        Item::Spawn(row, below) => {
            if last_thread.is_some_and(|l| i < l) {
                cv.put(3, yy, "│", LINE, None, false);
            }
            draw_spawn(cv, yy, row, below, view.frame, xr);
        }
        head => {
            cv.put(
                3,
                yy,
                if Some(i) == last_thread { "└" } else { "├" },
                LINE,
                None,
                false,
            );
            match head {
                Item::Bg(t) => draw_bg(cv, yy, t, i, view, xr),
                Item::Done => draw_done(cv, yy, d, view),
                Item::Thread(t, own) => draw_thread(cv, yy, t, own, view, xr),
                Item::Spawn(..) => {}
            }
        }
    }
}

/// An agent that spawned others, indented by depth, its children's glyphs on the right.
fn draw_spawn(cv: &mut Screen, yy: i32, row: &SpawnRow, below: &[&Agent], frame: f64, xr: i32) {
    let branch = format!("{}{}", row.prefix, if row.last { "└" } else { "├" });
    let x = cv.segs(5, yy, &[Seg::new(branch, LINE)]) + 1;
    let segs = agent_glyphs(below, frame, 5);
    let a = row.agent;
    let g = glyph(a, frame);
    let room = (xr - width(&segs) - 1 - (x + g.chars().count() as i32 + 1)).max(1);
    let name = trunc(
        &cleaned(&format!("{} {}", role_label(a), short_id(a))),
        room,
    );
    let att = a.status.needs_operator();
    let glyph_seg = Seg::new(g, glyph_colour(&a.status, frame))
        .bold_if(att)
        .fg_moving(att);
    cv.segs(x, yy, &[glyph_seg, Seg::plain(" "), Seg::new(name, FG)]);
    cv.segs_right(xr, yy, &segs);
}

/// A background task: its clock tile, what it does, and its kind and age on the right.
fn draw_bg(cv: &mut Screen, yy: i32, t: &BgTask, i: usize, view: &View, xr: i32) {
    let frame = view.frame;
    let kind = t.kind.to_lowercase();
    let when = match t.started {
        Some(s) => format!("{kind} {}", dur(Some(view.now - s))),
        None => kind,
    };
    let tl = tile(
        &Status::Background,
        &clock(frame, i as i64 + 1).to_string(),
        frame,
        None,
    );
    let room = (xr - when.chars().count() as i32 - 1 - (5 + tl.len() + 1)).clamp(1, 22);
    let desc = cleaned(&t.description);
    let desc = if desc.is_empty() {
        "in the background".to_owned()
    } else {
        desc
    };
    cv.segs(
        5,
        yy,
        &[tl, Seg::plain(" "), Seg::new(trunc(&desc, room), FG)],
    );
    cv.segs_right(xr, yy, &[Seg::new(when, DIM)]);
}

/// The done threads, folded into one row; unseen, its tile pulses.
fn draw_done(cv: &mut Screen, yy: i32, d: &DockState, view: &View) {
    let fm = mark_of(d.done.iter().flat_map(|t| &t.agents), &view.unseen, true);
    let n = format!("{} done", d.done.len());
    cv.segs(
        5,
        yy,
        &[
            tile(&Status::Done, "✓", view.frame, fm),
            Seg::plain(" "),
            Seg::new(n, FAINT),
        ],
    );
}

/// An active thread: its tile (unseen: its background pulses), name, and its own agents'
/// glyphs on the right.
fn draw_thread(cv: &mut Screen, yy: i32, t: &Thread, own: &[&Agent], view: &View, xr: i32) {
    let frame = view.frame;
    let (g, st) = thread_glyph(t, frame);
    let tl = tile(&st, &g, frame, mark_of(&t.agents, &view.unseen, true));
    let ags = agent_glyphs(own, frame, 4);
    let room = (xr - width(&ags) - 1 - (5 + tl.len() + 1)).clamp(1, 22);
    cv.segs(
        5,
        yy,
        &[
            tl,
            Seg::plain(" "),
            Seg::new(trunc(&cleaned(&t.name), room), FG),
        ],
    );
    cv.segs_right(xr, yy, &ags);
}
