//! What a view's frame needs from outside its state: the clocks, herdr's theme, panorama's
//! colour settings and the open key (Python `SwarmApp.repaint`'s reads).

use panorama_core::view::palette::{BG, DIM, DOCK_HOV, FAINT, FG};
use panorama_core::view::theme::OwnColours;
use panorama_core::view::{PanelSettings, Tokens};

/// panorama's own colours, which herdr's theme maps (the docked panel's look).
pub const OWN: OwnColours = OwnColours {
    fg: FG,
    dim: DIM,
    faint: FAINT,
    fallback: (DOCK_HOV, BG),
};

/// Everything a frame needs besides the view's own state.
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

impl Frame {
    /// The frame at the wall-clock instant `now`: its clock and its animation time, the same
    /// in every window at that instant (Python `SwarmApp.anim_frame`; tests/test_anim_sync.py).
    pub fn at(&mut self, now: f64) {
        self.now = now;
        self.frame = super::clock::frame_at(now);
    }
}
