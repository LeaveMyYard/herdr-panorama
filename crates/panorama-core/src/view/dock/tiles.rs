//! Design 9e's tiles (swarm-tiles.js, the operator's pick): a state is a coloured tile
//! `" g "`, as the graph's pods; a needs-you tile blinks to the off colours, and an unseen
//! one pulses its background toward the event's colour, never fully off (Q115). Also the
//! glyph runs of an entry's lines.

use super::state::{DockState, thread_glyph};
use crate::model::{Agent, Status};
use crate::view::anim::{blink_between, blink_faint, glyph, pulse};
use crate::view::colour::Rgb;
use crate::view::palette::{FAINT, TILE_OFF, status, tile as tile_colours};
use crate::view::seg::{Seg, width};
use crate::view::text::trunc;
use crate::view::unseen::Mark;

/// A state's tile, `" g "` in bold (Python `dock_tile`).
pub fn tile(st: &Status, g: &str, frame: f64, m: Option<Mark>) -> Seg {
    let (b, f) = tile_colours(st);
    let pulsed = |base: Rgb| m.map_or(base, |m| pulse(frame, m, base, 1.0));
    let (bg, fg) = if st.needs_operator() {
        (
            pulsed(blink_between(frame, b, TILE_OFF.0)),
            blink_between(frame, f, TILE_OFF.1),
        )
    } else {
        (pulsed(b), f)
    };
    let att = st.needs_operator();
    Seg::new(format!(" {g} "), fg)
        .bold()
        .on(bg)
        .fg_moving(att)
        .bg_moving(att || m.is_some())
}

/// A glyph's colour: a needs-you state blinks (Python `_gcol`).
pub fn glyph_colour(st: &Status, frame: f64) -> Rgb {
    if st.needs_operator() {
        blink_faint(frame, status(st))
    } else {
        status(st)
    }
}

/// One space between glyphs: some terminals draw a few glyphs wider than a cell, and
/// glyphs in adjacent cells overlapped (the operator's report).
pub fn spaced(segs: Vec<Seg>) -> Vec<Seg> {
    let mut out = Vec::with_capacity(segs.len() * 2);
    for (i, s) in segs.into_iter().enumerate() {
        if i > 0 {
            out.push(Seg::plain(" "));
        }
        out.push(s);
    }
    out
}

/// An entry's second line (swarm-tiles.js `rowTiles`): a tile per active thread, its most
/// urgent agent's state, and one `✓n` tile for the done threads.
pub fn thread_tiles(d: &DockState, frame: f64) -> Vec<Seg> {
    let mut tiles: Vec<Seg> = d
        .active
        .iter()
        .map(|t| {
            let (g, st) = thread_glyph(t, frame);
            tile(&st, &g, frame, None)
        })
        .collect();
    if !d.done.is_empty() {
        let n = if d.done.len() > 1 {
            d.done.len().to_string()
        } else {
            String::new()
        };
        tiles.push(tile(&Status::Done, &format!("✓{n}"), frame, None));
    }
    tiles
}

/// The thread tiles, then `counts`, in `room` cells. When they do not all fit, the first
/// tiles then `+n` for the rest, so the counts always stay whole (Python `_fit_thread_segs`).
pub fn fit_tiles(tiles: Vec<Seg>, counts: &str, room: i32) -> Vec<Seg> {
    let counts_len = counts.chars().count() as i32;
    let budget = room - counts_len;
    let all = spaced(tiles.clone());
    if width(&all) <= budget {
        return [all, vec![Seg::new(counts, FAINT)]].concat();
    }
    let more = |k: usize| format!("{}+{}", if k > 0 { " " } else { "" }, tiles.len() - k);
    let mut k = tiles.len();
    while k > 0
        && width(&spaced(tiles[..k].to_vec()))
            + (format!(" +{}", tiles.len() - k).chars().count() as i32)
            > budget
    {
        k -= 1;
    }
    let mut line = spaced(tiles[..k].to_vec());
    line.push(Seg::new(more(k), FAINT));
    line.push(Seg::new(counts, FAINT));
    if width(&line) <= room {
        line
    } else {
        vec![Seg::new(trunc(counts.trim(), room), FAINT)]
    }
}

/// The live agents' glyphs, most urgent first (up to `n`), a space between them, then
/// ` +N✓` for the finished ones (swarm-dock.js `agSegs`).
pub fn agent_glyphs(agents: &[&Agent], frame: f64, n: usize) -> Vec<Seg> {
    let mut live: Vec<&&Agent> = agents.iter().filter(|a| a.status.is_live()).collect();
    live.sort_by_key(|a| a.status.urgency());
    let fin = agents.len() - live.len();
    let glyphs = live
        .iter()
        .take(n)
        .map(|a| Seg::new(glyph(a, frame), status(&a.status)).bold_if(a.status.needs_operator()))
        .collect();
    let mut segs = spaced(glyphs);
    if fin > 0 {
        segs.push(Seg::new(format!(" +{fin}✓"), FAINT));
    }
    segs
}
