//! The view's own state: what is selected, hovered, filtered and how the panel looks
//! (Python `render.View`, the part the docked panel uses; the graph's columns join in P4).

use super::filter::Filter;
use super::target::Target;
use super::theme::PanelLook;
use super::unseen::Marks;
use std::collections::{BTreeMap, BTreeSet};

/// The orchestrator-list id of the `+ New agent` row (Python `NEW_ROW`).
pub const NEW_ROW: &str = "new:";

/// How a view is drawn this frame. The binary owns it and passes it to each render.
#[derive(Clone, Debug)]
pub struct View {
    /// The selected orchestrator's pane id, or [`NEW_ROW`].
    pub orch: Option<String>,
    pub filter: Filter,
    /// The unseen marks.
    pub unseen: Marks,
    /// The card under the mouse.
    pub hover: Option<Target>,
    /// Docked next to an agent in the home pane: `z` zooms to the graph (hinted in the rule).
    pub zoom_hint: bool,
    /// The open key as herdr binds it, (full, short) (`openkey.py`).
    pub open_key: (String, String),
    /// The keyboard cursor wears the hover look. A per-tab panel turns it off: its selected
    /// row is the shown orchestrator, already docked there.
    pub cursor_look: bool,
    /// The docked panel in herdr's theme, focused or not; `None`: panorama's own colours.
    pub panel: Option<PanelLook>,
    /// The time now (POSIX seconds), for ages.
    pub now: f64,
    /// Seconds east of UTC for clock times (the caller looks it up).
    pub utc_offset: i64,
    /// The animation time in design frames of 250 ms (see `anim`).
    pub frame: f64,
    /// Per column: its scroll offset.
    pub offsets: BTreeMap<usize, i32>,
    /// Columns the wheel scrolled: they no longer follow the selection.
    pub wheel: BTreeSet<usize>,
    /// The empty page's words when herdr shows no agent session. The caller names its
    /// harness here (the core is harness-neutral); empty by default.
    pub no_sessions: String,
}

impl Default for View {
    fn default() -> Self {
        Self {
            orch: None,
            filter: Filter::All,
            unseen: Marks::new(),
            hover: None,
            zoom_hint: false,
            open_key: ("pfx+spc".into(), "pfx+spc".into()),
            cursor_look: true,
            panel: None,
            now: 0.0,
            utc_offset: 0,
            frame: 0.0,
            offsets: BTreeMap::new(),
            wheel: BTreeSet::new(),
            no_sessions: String::new(),
        }
    }
}
