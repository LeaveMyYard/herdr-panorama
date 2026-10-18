//! The panel's state, as the core's messages change it (Python `PanelApp._got_state`,
//! `_status`, `herdr_focus`, and the parts of `SwarmApp.apply` a panel uses). No IO: the
//! actor feeds it and paints it.
//!
//! The panel's one selected row is the orchestrator herdr shows (`state.visible_tabs`, else
//! the one whose pane has the focus), the same in every tab's panel. A filter that hides it
//! leaves no row selected, and it is selected again as soon as the filter shows it.

mod selection;

use crate::proto::state::State;
use panorama_core::model::Swarm;
use panorama_core::view::nav::default_children;
use panorama_core::view::{Filter, Marks, View};
use std::collections::HashMap;

/// One per-tab panel.
#[derive(Debug)]
pub struct Panel {
    /// This panel's pane and tab (`$HERDR_PANE_ID`, `$HERDR_TAB_ID`).
    pub my_pane: Option<String>,
    pub my_tab: Option<String>,
    /// The last swarm the core sent (an empty one before the first).
    pub swarm: Swarm,
    /// A state has arrived.
    pub loaded: bool,
    /// A core is connected (welcomed).
    pub connected: bool,
    /// What is drawn: the selection, the filter, the unseen marks.
    pub view: View,
    /// herdr shows this panel's tab (a hidden panel draws no animation frames).
    pub tab_shown: bool,
    /// This panel's pane has the keyboard: the focused look.
    pub pane_focused: bool,
    /// Tab id -> its panel's pane, where a switch to that tab puts the keyboard.
    pub panes_of: HashMap<String, String>,
    herdr_focused_pane: Option<String>,
    /// The orchestrator herdr shows, as last applied to the selection.
    active: Option<String>,
    /// Where the arrows resume from while no row is selected.
    parked: Option<String>,
}

impl Panel {
    /// A panel in `my_pane` of `my_tab`, before any core.
    pub fn new(my_pane: Option<String>, my_tab: Option<String>) -> Self {
        Self {
            my_pane,
            my_tab,
            swarm: Swarm::default(),
            loaded: false,
            connected: false,
            view: View {
                cursor_look: false,
                ..View::default()
            },
            tab_shown: true,
            pane_focused: true,
            panes_of: HashMap::new(),
            herdr_focused_pane: None,
            active: None,
            parked: None,
        }
    }

    /// A state from the core.
    pub fn on_state(&mut self, st: State) {
        let mut swarm = st.swarm;
        let mine = swarm
            .orchestrators
            .iter()
            .find(|o| o.tab_id.is_some() && o.tab_id == self.my_tab)
            .map(|o| o.pane_id.clone());
        swarm.docked = mine.clone(); // this tab's orchestrator: the panel's docked entry
        self.tab_shown = match (&st.visible_tabs, &self.my_tab) {
            (Some(vis), Some(tab)) => vis.contains(tab),
            _ => true,
        };
        self.panes_of = st.panels;
        let active = selection::herdr_shows(
            &swarm,
            st.visible_tabs.as_deref(),
            st.focused_pane.as_deref(),
        );
        let first = !self.loaded;
        self.apply(
            swarm,
            st.unseen,
            if first { active.clone().or(mine) } else { None },
        );
        if let Some(f) = st.filter {
            self.core_filter(f);
        }
        match active {
            Some(a) if self.active.as_ref() != Some(&a) => {
                // one selected row in every tab's panel: the orchestrator herdr shows; a row
                // an arrow moved to stays until the shown one changes
                if !self.shown_ids().contains(&a) {
                    self.view.orch = None;
                    self.view.focus = 0;
                    self.parked = Some(a.clone());
                } else if self.view.orch.as_ref() != Some(&a) {
                    self.view.orch = Some(a.clone());
                    self.view.focus = 0;
                    default_children(&self.swarm, &mut self.view);
                }
                self.active = Some(a);
            }
            _ => {
                self.back_to_shown();
            }
        }
        if let Some(f) = st.focused_pane {
            self.herdr_focus(&f);
        }
    }

    /// The model and its marks (Python `SwarmApp.apply`): the first selects `focus_pane`
    /// (else the first row); later ones keep the selection while it is shown.
    fn apply(&mut self, swarm: Swarm, marks: Marks, focus_pane: Option<String>) {
        self.swarm = swarm;
        self.view.unseen = marks;
        if !self.loaded {
            self.loaded = true;
            let ids = self.shown_ids();
            self.view.orch = focus_pane
                .filter(|p| ids.contains(p))
                .or_else(|| ids.first().cloned());
            self.view.focus = 0;
            default_children(&self.swarm, &mut self.view);
        } else {
            self.resolve();
        }
    }

    /// The core's filter (1.3; an older core sends none and this panel's own stays).
    fn core_filter(&mut self, f: Filter) {
        if f != self.view.filter {
            self.apply_filter(f);
        }
    }

    /// herdr says `pane` got the focus: this panel is focused exactly when it is its own pane.
    pub fn herdr_focus(&mut self, pane: &str) {
        self.herdr_focused_pane = Some(pane.to_owned());
        if let Some(mine) = &self.my_pane
            && !pane.is_empty()
        {
            self.pane_focused = pane == mine;
        }
    }

    /// The terminal reports focus in or out.
    pub fn on_terminal_focus(&mut self, focused: bool) {
        self.pane_focused = focused;
    }

    /// A core connected (welcomed), or the connection ended.
    pub fn on_connected(&mut self, on: bool) {
        self.connected = on;
    }
}

#[cfg(test)]
mod tests;
