//! A frame of the panel (Python `SwarmApp.repaint` plus `PanelApp.decorate`): the docked view
//! of its tab's orchestrator in herdr's theme, focused or not; the graph when its tab has
//! none (Python's `_update_dock`): the narrow layout, or the wide one from 120 columns;
//! dimmed with the notice while there is no core.

use super::model::Panel;
pub use crate::ui::frame::Frame;
use crate::ui::frame::OWN;
use panorama_core::view::palette::{BG, DIM, FG};
use panorama_core::view::theme::panel_look;
use panorama_core::view::{Screen, render_dock, render_graph};

/// The operator's notice while there is no core (Q139).
pub const NOTICE: &str = "panorama is restarting…";
/// How far a disconnected panel's colours go toward the background.
pub const DIM_BY: f64 = 0.5;
/// The empty page's words: this panel follows Claude Code sessions.
pub const NO_SESSIONS: &str = "No Claude Code sessions in herdr";

impl Panel {
    /// The frame at `f`, `w` × `h`. The list's scroll offset is kept for the next frame.
    pub fn paint(&mut self, w: i32, h: i32, f: &Frame) -> Screen {
        self.input.width = w;
        self.update_dock();
        let docked = self.input.dock;
        let v = &mut self.view;
        v.now = f.now;
        v.frame = f.frame;
        v.utc_offset = f.utc_offset;
        v.open_key = f.open_key.clone();
        let stale = self.swarm.errors.first().map(String::as_str);
        let mut s = if docked {
            v.panel = Some(panel_look(
                f.theme.as_ref(),
                self.pane_focused,
                OWN,
                &f.settings,
            ));
            render_dock(&self.swarm, v, w, h, stale, NO_SESSIONS)
        } else {
            v.panel = None;
            render_graph(&self.swarm, v, w, h, stale, NO_SESSIONS)
        };
        for col in &s.columns {
            v.offsets.insert(col.column, col.offset);
        }
        self.decorate(&mut s);
        s
    }

    /// Without a core: the last picture, dimmed (animated colours as drawn), with the notice
    /// on the first line; before any state, the notice over the empty page.
    fn decorate(&self, s: &mut Screen) {
        if self.connected {
            return;
        }
        if !self.loaded {
            s.put(1, 0, NOTICE, DIM, None, false);
            return;
        }
        for c in s.cells_mut() {
            if !c.fg_moves {
                c.fg = c.fg.mix(BG, DIM_BY);
            }
        }
        s.put(
            0,
            0,
            &" ".repeat(s.w().max(0) as usize),
            FG,
            Some(BG),
            false,
        );
        s.put(1, 0, NOTICE, FG, None, false);
    }
}
