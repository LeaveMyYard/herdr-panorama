//! The pages shown instead of any swarm view (Python `render.render`'s first branches): herdr
//! not reachable, or no sessions. They wear the graph's chrome: its header, legend and keys.

mod chrome;

use super::palette::{BG, DIM, FAINT, status};
use super::screen::Screen;
use super::seg::Seg;
use super::state::View;
use crate::model::{Status, Swarm};

/// At least this wide, the graph's wide layout (and its wide chrome).
pub const WIDE: i32 = 120;

/// The empty page for `swarm`, `w` × `h`.
pub fn page(swarm: &Swarm, view: &View, w: i32, h: i32, stale: Option<&str>) -> Screen {
    let mut s = Screen::new(w, h);
    s.fill_bg(0, 0, w, h, BG);
    chrome::header(&mut s, swarm, view, stale);
    if !swarm.herdr_ok {
        let msg = Seg::new("herdr is not reachable", status(&Status::Waiting)).bold();
        s.segs(2, 2, &[msg, Seg::new("  (retrying)", FAINT)]);
    } else {
        s.segs(2, 2, &[Seg::new(view.no_sessions.as_str(), DIM)]);
    }
    chrome::legend(&mut s, h - 3);
    chrome::keys(&mut s, view.zoom_hint, swarm.inbox_ok);
    s
}
