//! The views, rendered into a cell grid (plan §2.3, `panorama-core::view`).
//!
//! A render is a pure function of the swarm, the [`View`] and the size: it returns a
//! [`Screen`] with its click targets. Animation is a function of `View.frame`, so the caller
//! renders again on each colour tick (see `anim`).
//!
//! The docked panel ([`render_dock`]) is the per-tab panel's view (P3); the graph and the
//! narrow layout join in P4.

pub mod anim;
pub mod chrome;
pub mod colour;
pub mod dock;
pub mod empty;
pub mod filter;
pub mod orch_state;
pub mod palette;
pub mod screen;
pub mod seg;
pub mod state;
pub mod target;
pub mod text;
pub mod theme;
pub mod unseen;

pub use colour::Rgb;
pub use filter::Filter;
pub use screen::{Cell, Screen, ScrollRegion};
pub use state::{NEW_ROW, View};
pub use target::Target;
pub use theme::{PanelLook, PanelSettings, Tokens};
pub use unseen::{Marks, Unseen};

use crate::model::Swarm;

/// The docked panel as Python's `render(…)` draws it with `view.dock` on: herdr not reachable
/// and no sessions are the graph's empty pages; otherwise [`dock::dock_panel`]. `stale`: why
/// the model may be out of date (a failed refresh). `no_sessions`: the empty page's words when
/// herdr shows no agent session; the caller names its harness (required: the core names none).
///
/// Every string in `swarm` (and `stale`) is drawn as given: the caller passes them sanitized
/// (the panel cleans the core's state as it reads it), so no control character reaches the
/// terminal.
pub fn render_dock(
    swarm: &Swarm,
    view: &View,
    w: i32,
    h: i32,
    stale: Option<&str>,
    no_sessions: &str,
) -> Screen {
    debug_assert!(!no_sessions.is_empty(), "the empty page needs its words");
    if !swarm.herdr_ok || swarm.orchestrators.is_empty() {
        return empty::page(swarm, view, w, h, stale, no_sessions);
    }
    dock::dock_panel(swarm, view, w, h, stale.is_some())
}
