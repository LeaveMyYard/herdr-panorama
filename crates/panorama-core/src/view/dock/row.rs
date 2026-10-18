//! One orchestrator's entry in the docked list (swarm-dock.js `vA` with 9e's tiles, turn 8's
//! waiting and stale; Python `render.dock_row`): tab number, state tile, name, `?N` and its
//! state; then its threads' tiles and counts, or, for the docked entry, only the counts and
//! its list below (`items`).

use super::items::{Item, draw_item, items_of};
use super::state::{dock_state, state_colour};
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
use herdr_agent_kit::herdr_api::{MAX_ARG, clean};

/// How the entry is drawn: next to its agent (docked), under the keyboard cursor.
#[derive(Clone, Copy, Debug)]
pub struct RowLook {
    pub docked: bool,
    pub cursor: bool,
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
    let frame = view.frame;
    let d = dock_state(o, frame, ages);
    let items: Vec<Item> = if look.docked { items_of(o, &d) } else { vec![] };
    let hov = !look.docked
        && ((look.cursor && view.cursor_look)
            || view.hover == Some(Target::Orch(o.pane_id.clone())));
    let h = 2 + items.len() as i32;
    let xr = w - 1; // right-aligned fields end at the real right edge
    cv.area(0, w, y, y + h, Target::Orch(o.pane_id.clone()));
    // unseen: the entry's two lines pulse as a tint
    let m = mark_of(o.agents(), &view.unseen, true);
    let base = if look.docked {
        SEL
    } else if hov {
        DOCK_HOV
    } else {
        BG
    };
    if look.docked || hov || m.is_some() {
        cv.fill_bg_moving(
            0,
            y,
            w,
            2,
            m.map_or(base, |m| tint(frame, m, base)),
            m.is_some(),
        );
    }
    if look.docked {
        for j in 0..h {
            cv.put(0, y + j, "▌", KEY, None, false);
        }
    } else if hov {
        cv.put(0, y, "›", DIM, None, false);
    }
    let mut right = vec![];
    if d.q > 0 {
        right.push(Seg::new(format!("?{}", d.q), status(&Status::Waiting)).bold());
        right.push(Seg::plain(" "));
    }
    right.push(Seg::new(d.label.clone(), state_colour(&d)));
    let tab = o.tab_no.map_or_else(|| "?".to_owned(), |n| n.to_string());
    let state = tile(&d.st, &d.glyph, frame, None);
    let x_name = 2 + tab.chars().count() as i32 + 1 + state.len() + 1;
    // a narrow pane cuts the name, never the state
    let room = (xr - width(&right) - 1 - x_name).clamp(1, 22);
    let name_fg = if look.docked || hov {
        HI
    } else if d.st == Status::Stale {
        FAINT
    } else if d.st == Status::Done {
        DIM
    } else {
        FG
    };
    let name = trunc(&clean(&o.tab_label, MAX_ARG), room);
    cv.segs(
        2,
        y,
        &[
            Seg::new(tab, FAINT),
            Seg::plain(" "),
            state,
            Seg::plain(" "),
            Seg::new(name, name_fg).bold_if(look.docked),
        ],
    );
    cv.segs_right(xr, y, &right);
    if d.q > 0 {
        let xq = xr - width(&right);
        cv.target(
            xq,
            xq + 1 + d.q.to_string().len() as i32,
            y,
            Target::AnswerOrch(o.pane_id.clone()),
        );
    }
    let extra = if d.st == Status::Background {
        format!(" · {} bg", d.bg)
    } else {
        String::new()
    };
    let counts = format!("{} active · {} agents{extra}", d.active.len(), d.live);
    if items.is_empty() {
        let tiles = thread_tiles(&d, frame);
        let lead = if tiles.is_empty() { "" } else { "  " };
        cv.segs(
            x_name,
            y + 1,
            &fit_tiles(tiles, &format!("{lead}{counts}"), w - x_name),
        );
    } else {
        // 9e: the docked entry's list shows the thread tiles, so its line 2 keeps only the counts
        cv.segs(
            x_name,
            y + 1,
            &[Seg::new(trunc(&counts, w - x_name), FAINT)],
        );
    }
    let last_thread = items.iter().rposition(Item::is_head);
    for (i, item) in items.iter().enumerate() {
        draw_item(cv, y + 2 + i as i32, item, i, last_thread, &d, view, xr);
    }
    h
}
