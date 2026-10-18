//! A frame of the panel (Python `SwarmApp.repaint` plus `PanelApp.decorate`): the docked view
//! of its tab's orchestrator in herdr's theme, focused or not; the narrow layout when its tab
//! has none (Python's `_update_dock`); dimmed with the notice while there is no core.
//!
//! Python draws the wide graph at 120 columns and more (P4); a panel that wide shows the
//! narrow layout here, a stated gap until P4 (panels are 46 columns).

use super::model::Panel;
use panorama_core::view::palette::{BG, DIM, DOCK_HOV, FAINT, FG};
use panorama_core::view::theme::{OwnColours, panel_look};
use panorama_core::view::{PanelSettings, Screen, Tokens, render_dock, render_narrow};

/// The operator's notice while there is no core (Q139).
pub const NOTICE: &str = "panorama is restarting…";
/// How far a disconnected panel's colours go toward the background.
pub const DIM_BY: f64 = 0.5;
/// The empty page's words: this panel follows Claude Code sessions.
pub const NO_SESSIONS: &str = "No Claude Code sessions in herdr";

const OWN: OwnColours = OwnColours {
    fg: FG,
    dim: DIM,
    faint: FAINT,
    fallback: (DOCK_HOV, BG),
};

/// Everything a frame needs besides the panel's state.
#[derive(Clone, Debug)]
pub struct Frame {
    /// The wall clock (POSIX seconds) and the animation time in frames of 250 ms.
    pub now: f64,
    pub frame: f64,
    /// Seconds east of UTC, for the clock.
    pub utc_offset: i64,
    /// herdr's theme, panorama's colour settings, and the open key as herdr binds it.
    pub theme: Option<Tokens>,
    pub settings: PanelSettings,
    pub open_key: (String, String),
}

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
            render_narrow(&self.swarm, v, w, h, stale, NO_SESSIONS)
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
