//! The view's own state: what is selected, hovered, filtered and how the panel looks
//! (Python `render.View`: the parts the docked panel and the narrow layout use; the wide
//! graph's sideways scroll joins in P4).

use super::filter::Filter;
use super::target::Target;
use super::theme::PanelLook;
use super::unseen::Marks;
use std::collections::{BTreeMap, BTreeSet};

/// The orchestrator-list id of the `+ New agent` row (Python `NEW_ROW`).
pub const NEW_ROW: &str = "new:";
/// The thread-list id prefix of an orchestrator's "n complete" card (`DONE_CARD` + pane id).
pub const DONE_CARD: &str = "done:";
/// The thread-list id prefix of an orchestrator's background card (`BG_CARD` + pane id).
pub const BG_CARD: &str = "bg:";
/// The agent-list id prefix of a "n finished" card (`FIN_CARD` + group key).
pub const FIN_CARD: &str = "fin:";

/// How a view is drawn this frame. The binary owns it and passes it to each render.
#[derive(Clone, Debug)]
pub struct View {
    /// The selected orchestrator's pane id, or [`NEW_ROW`].
    pub orch: Option<String>,
    /// The selected thread's id (or [`DONE_CARD`] / [`BG_CARD`] + the pane id).
    pub thread: Option<String>,
    /// The selected agent's id at any depth (or [`FIN_CARD`] + its group key).
    pub agent: Option<String>,
    /// The column the keyboard is in: 0 orchestrators, 1 threads, 2.. agents by depth.
    pub focus: usize,
    /// Pane ids whose done threads are unfolded.
    pub done_open: BTreeSet<String>,
    /// Group keys whose finished agents are unfolded.
    pub fin_open: BTreeSet<String>,
    /// Pane id -> the thread last selected under it; group key -> the agent last selected.
    pub remember_thread: BTreeMap<String, String>,
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
}

impl Default for View {
    fn default() -> Self {
        Self {
            orch: None,
            thread: None,
            agent: None,
            focus: 0,
            done_open: BTreeSet::new(),
            fin_open: BTreeSet::new(),
            remember_thread: BTreeMap::new(),
            remember_agent: BTreeMap::new(),
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
        }
    }
}
