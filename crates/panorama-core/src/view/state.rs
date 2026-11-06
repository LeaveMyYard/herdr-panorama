//! The view's own state: what is selected, hovered, filtered and how the panel looks
//! (Python `render.View`).

use super::filter::Filter;
use super::target::Target;
use super::theme::PanelLook;
use super::unseen::Marks;
use std::collections::{BTreeMap, BTreeSet};

/// The root-list id of the `+ New agent` row (Python `NEW_ROW`).
pub const NEW_ROW: &str = "new:";
/// The agent-list id prefix of a "n finished" card (`FIN_CARD` + group key: the id of the
/// agent that spawned the group, or the root's pane id for the root's own).
pub const FIN_CARD: &str = "fin:";

/// How a view is drawn this frame. The binary owns it and passes it to each render.
#[derive(Clone, Debug)]
pub struct View {
    /// The selected root's pane id, or [`NEW_ROW`].
    pub orch: Option<String>,
    /// The selected agent's id at any depth (or [`FIN_CARD`] + its group key).
    pub agent: Option<String>,
    /// The column the keyboard is in: its depth (0 the roots, 1.. their agents by depth).
    pub focus: usize,
    /// Group keys whose finished agents are unfolded.
    pub fin_open: BTreeSet<String>,
    /// Group key -> the agent last selected in it.
    pub remember_agent: BTreeMap<String, String>,
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
    /// row is the shown root, already docked there.
    pub cursor_look: bool,
    /// The docked panel in herdr's theme, focused or not; `None`: panorama's own colours.
    pub panel: Option<PanelLook>,
    /// Harness title presentation, resolved by the IO boundary.
    pub harness_icons: super::harness::HarnessIcons,
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
    /// The wide graph's first column shown, set by a sideways scroll; `None`: follow the
    /// focus. A render reports the column it showed first in [`Screen::hlo`](super::Screen).
    pub hcol: Option<usize>,
}

impl Default for View {
    fn default() -> Self {
        Self {
            orch: None,
            agent: None,
            focus: 0,
            fin_open: BTreeSet::new(),
            remember_agent: BTreeMap::new(),
            filter: Filter::All,
            unseen: Marks::new(),
            hover: None,
            zoom_hint: false,
            open_key: ("pfx+spc".into(), "pfx+spc".into()),
            cursor_look: true,
            panel: None,
            harness_icons: Default::default(),
            now: 0.0,
            utc_offset: 0,
            frame: 0.0,
            offsets: BTreeMap::new(),
            wheel: BTreeSet::new(),
            hcol: None,
        }
    }
}
