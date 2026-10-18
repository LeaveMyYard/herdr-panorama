//! A frame of the app (Python `SwarmApp.repaint`): the docked panel in herdr's theme when
//! docked and narrow, else the graph (wide from 120 columns, narrow below), with the last
//! refresh's error in the header.

use super::model::App;
use crate::ui::frame::{Frame, OWN};
use panorama_core::view::theme::panel_look;
use panorama_core::view::{Screen, render_dock, render_graph};

/// The empty page's words: this view follows Claude Code sessions.
pub const NO_SESSIONS: &str = "No Claude Code sessions in herdr";

impl App {
    /// The frame at `f`, `w` × `h`; `None` while the pane has no size (nothing is painted).
    pub fn paint(&mut self, w: i32, h: i32, f: &Frame) -> Option<Screen> {
        if !self.before_paint(w, h) {
            return None;
        }
        let stale = self.stale().map(str::to_owned);
        let stale = stale.as_deref();
        let v = &mut self.graph.view;
        v.now = f.now;
        v.frame = f.frame;
        v.utc_offset = f.utc_offset;
        v.open_key = f.open_key.clone();
        let s = if self.dock {
            let look = panel_look(f.theme.as_ref(), self.pane_focused, OWN, &f.settings);
            v.panel = Some(look);
            render_dock(&self.graph.swarm, v, w, h, stale, NO_SESSIONS)
        } else {
            v.panel = None;
            render_graph(&self.graph.swarm, v, w, h, stale, NO_SESSIONS)
        };
        self.painted(&s);
        Some(s)
    }
}

#[cfg(test)]
mod tests;
