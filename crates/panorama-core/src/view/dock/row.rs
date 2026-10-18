//! One orchestrator's entry in the docked list (swarm-dock.js `vA` with 9e's tiles, turn 8's
//! waiting and stale; Python `render.dock_row`): its background, line 1 (tab number, state
//! tile, name, `?N` and its state), line 2 (its threads' tiles and counts, or for the docked
//! entry only the counts), and the docked entry's list below (`items`).

use super::items::{Item, ItemCtx, draw_item, items_of};
use super::state::{DockState, dock_state, state_colour};
use super::tiles::{fit_tiles, thread_tiles, tile};
use crate::model::{Orchestrator, Status};
use crate::view::anim::tint;
use crate::view::orch_state::Ages;
use crate::view::palette::{BG, DIM, DOCK_HOV, FAINT, FG, HI, KEY, SEL, status};
use crate::view::screen::Screen;
use crate::view::seg::{Seg, width};
use crate::view::state::View;
use crate::view::target::Target;
use crate::view::text::trunc;
use crate::view::unseen::mark_of;

/// How the entry is drawn: next to its agent (docked), under the keyboard cursor.
#[derive(Clone, Copy, Debug)]
pub struct RowLook {
    pub docked: bool,
    pub cursor: bool,
}

/// A name never takes more than this many cells.
const NAME_MAX: i32 = 22;

/// One entry being drawn at row `y`, `w` cells wide.
struct Entry<'a> {
    o: &'a Orchestrator,
    d: DockState<'a>,
    items: Vec<Item<'a>>,
    docked: bool,
    /// The hover look: the mouse, or the cursor where the cursor wears it.
    hov: bool,
    y: i32,
    w: i32,
}

impl Entry<'_> {
    fn height(&self) -> i32 {
        2 + self.items.len() as i32
    }

    /// Right-aligned fields end at the real right edge.
    fn xr(&self) -> i32 {
        self.w - 1
    }
}

/// Draws `o`'s entry at row `y` of `cv`, `w` cells wide; its height.
pub fn dock_row(
    cv: &mut Screen,
    y: i32,
    o: &Orchestrator,
    view: &View,
    ages: Ages,
    look: RowLook,
    w: i32,
) -> i32 {
    let d = dock_state(o, view.frame, ages);
    let items = if look.docked { items_of(o, &d) } else { vec![] };
    let hov = !look.docked
        && ((look.cursor && view.cursor_look)
            || view.hover == Some(Target::Orch(o.pane_id.clone())));
    let e = Entry {
        o,
        d,
        items,
        docked: look.docked,
        hov,
        y,
        w,
    };
    cv.area(0, w, y, y + e.height(), Target::Orch(o.pane_id.clone()));
    background(cv, &e, view);
    let x_name = first_line(cv, &e, view);
    second_line(cv, &e, view, x_name);
    let ctx = ItemCtx {
        view,
        d: &e.d,
        xr: e.xr(),
        last_head: e.items.iter().rposition(Item::is_head),
    };
    for (i, item) in e.items.iter().enumerate() {
        draw_item(cv, y + 2 + i as i32, item, i, &ctx);
    }
    e.height()
}

/// The entry's look: the docked or hover background on its two lines (an unseen mark pulses
/// there as a tint), and the docked bar down its whole height or the hover mark.
fn background(cv: &mut Screen, e: &Entry, view: &View) {
    let m = mark_of(e.o.agents(), &view.unseen, true);
    let base = if e.docked {
        SEL
    } else if e.hov {
        DOCK_HOV
    } else {
        BG
    };
    if e.docked || e.hov || m.is_some() {
        cv.fill_bg_moving(
            0,
            e.y,
            e.w,
            2,
            m.map_or(base, |m| tint(view.frame, m, base)),
            m.is_some(),
        );
    }
    if e.docked {
        for j in 0..e.height() {
            cv.put(0, e.y + j, "▌", KEY, None, false);
        }
    } else if e.hov {
        cv.put(0, e.y, "›", DIM, None, false);
    }
}

/// Line 1: tab number, state tile, the name (a narrow pane cuts the name, never the state),
/// and `?N` (a click target: answer) plus the state on the right. Where the name starts.
fn first_line(cv: &mut Screen, e: &Entry, view: &View) -> i32 {
    let d = &e.d;
    let mut right = vec![];
    if d.q > 0 {
        right.push(Seg::new(format!("?{}", d.q), status(&Status::Waiting)).bold());
        right.push(Seg::plain(" "));
    }
    right.push(Seg::new(d.label.clone(), state_colour(d)));
    // Python's `or`: no tab number, or 0, reads `?`
    let tab =
        e.o.tab_no
            .filter(|n| *n != 0)
            .map_or_else(|| "?".to_owned(), |n| n.to_string());
    let state = tile(&d.st, &d.glyph, view.frame, None);
    let x_name = 2 + tab.chars().count() as i32 + 1 + state.len() + 1;
    let room = (e.xr() - width(&right) - 1 - x_name).clamp(1, NAME_MAX);
    let name_fg = if e.docked || e.hov {
        HI
    } else if d.st == Status::Stale {
        FAINT
    } else if d.st == Status::Done {
        DIM
    } else {
        FG
    };
    let name = Seg::new(trunc(&e.o.tab_label, room), name_fg).bold_if(e.docked);
    cv.segs(
        2,
        e.y,
        &[
            Seg::new(tab, FAINT),
            Seg::plain(" "),
            state,
            Seg::plain(" "),
            name,
        ],
    );
    cv.segs_right(e.xr(), e.y, &right);
    if d.q > 0 {
        let xq = e.xr() - width(&right);
        cv.target(
            xq,
            xq + 1 + d.q.to_string().len() as i32,
            e.y,
            Target::AnswerOrch(e.o.pane_id.clone()),
        );
    }
    x_name
}

/// Line 2: the threads' tiles then the counts; for the docked entry (whose list shows the
/// thread tiles) only the counts (9e).
fn second_line(cv: &mut Screen, e: &Entry, view: &View, x_name: i32) {
    let d = &e.d;
    let extra = if d.st == Status::Background {
        format!(" · {} bg", d.bg)
    } else {
        String::new()
    };
    let counts = format!("{} active · {} agents{extra}", d.active.len(), d.live);
    let room = e.w - x_name;
    if !e.items.is_empty() {
        cv.segs(x_name, e.y + 1, &[Seg::new(trunc(&counts, room), FAINT)]);
        return;
    }
    let tiles = thread_tiles(d, view.frame);
    let lead = if tiles.is_empty() { "" } else { "  " };
    cv.segs(
        x_name,
        e.y + 1,
        &fit_tiles(tiles, &format!("{lead}{counts}"), room),
    );
}
