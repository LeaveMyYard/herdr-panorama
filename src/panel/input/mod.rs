//! The panel's input (Python `SwarmApp`'s keys, mouse and arming, with `PanelApp`'s
//! overrides): what a key or a click does to the selection, and the [`Effect`]s the actor
//! then carries out (a switch, the inbox, a toast). No IO: every method takes the time.
//!
//! - [`keys`]: keys to actions; [`moves`]: the cursor; [`actions`]: what the action keys
//!   do; [`arrows`]: the docked list's arrows switching the chats once they rest;
//!   [`mouse`]: clicks, hover and the wheel; [`arming`]: when keys and clicks act.

pub mod actions;
pub mod arming;
pub mod arrows;
pub mod effect;
pub mod filter;
#[cfg(test)]
mod fixtures;
pub mod keys;
pub mod mouse;
pub mod moves;
pub mod selection;

pub use arming::{ClickArming, Painted};
pub use effect::{Effect, Toast};
pub use keys::Key;

use panorama_core::view::Filter;
use std::time::{Duration, Instant};

/// No key acts this long after what it would act on changed (Python `ARM_SECONDS`).
pub const ARM: Duration = Duration::from_millis(400);
/// A filter chosen here wins over the core's older word this long.
pub const FILTER_ECHO: Duration = Duration::from_secs(1);
/// The docked list: switch once the arrows rest this long (holding one switches once).
pub const ARROW_REST: Duration = Duration::from_millis(200);

/// The settings the input reads (panorama's `config.toml`, `cockpit.py`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputSettings {
    /// `dock_keep_focus`: a switch from the docked list leaves the keyboard on the panel.
    pub keep_focus: bool,
    /// `dock_on_arrows`: the arrows switch the chats; only with `keep_focus` (an arrow never
    /// moves the keyboard into an agent; Python `cockpit.arrows_setting`).
    pub arrows: bool,
}

impl Default for InputSettings {
    fn default() -> Self {
        Self {
            keep_focus: true,
            arrows: true,
        }
    }
}

/// What the input keeps between events.
#[derive(Debug)]
pub struct InputState {
    /// Keys act from then on.
    pub armed_at: Instant,
    /// A filter chosen here, until the core echoes it.
    pub filter_sent: Option<(Filter, Instant)>,
    /// A switch is under way (one at a time).
    pub docking: bool,
    /// The arrows came to rest during a switch: look again when it ends.
    pub arrow_again: bool,
    /// When the arrows' switch is due.
    pub arrow_due: Option<Instant>,
    /// Docked (the list has one column), as decided on the last refresh or paint.
    pub dock: bool,
    /// The pane's width at the last paint (0 before the first).
    pub width: i32,
    pub settings: InputSettings,
    /// Per-cell click arming.
    pub clicks: ClickArming,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            armed_at: Instant::now(),
            filter_sent: None,
            docking: false,
            arrow_again: false,
            arrow_due: None,
            dock: false,
            width: 0,
            settings: InputSettings::default(),
            clicks: ClickArming::default(),
        }
    }
}
