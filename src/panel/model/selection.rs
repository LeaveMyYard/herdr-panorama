//! The panel's selected row (Python `PanelApp._herdr_shows`, `_apply_filter`,
//! `_back_to_shown`, and `SwarmApp.resolve`, `_filter_to`).

use super::Panel;
use panorama_core::model::Swarm;
use panorama_core::view::filter::visible_orchs;
use panorama_core::view::nav::{default_children, nav};
use panorama_core::view::{Filter, NEW_ROW};

/// The local orchestrator in the tab herdr shows, else the one whose pane has the focus.
pub(super) fn herdr_shows(
    swarm: &Swarm,
    visible: Option<&[String]>,
    focused: Option<&str>,
) -> Option<String> {
    let local = || swarm.orchestrators.iter().filter(|o| o.machine.is_none());
    if let Some(vis) = visible {
        let hit = local().find(|o| o.tab_id.as_ref().is_some_and(|t| vis.contains(t)));
        if let Some(o) = hit {
            return Some(o.pane_id.clone());
        }
    }
    let focused = focused?;
    local()
        .find(|o| o.pane_id == focused)
        .map(|o| o.pane_id.clone())
}

impl Panel {
    /// The selectable rows, in order: the orchestrators the filter shows, then `+ New agent`
    /// (Python `render.nav(...)[0]`).
    pub fn shown_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = visible_orchs(&self.swarm, &self.view)
            .iter()
            .map(|o| o.pane_id.clone())
            .collect();
        ids.push(NEW_ROW.to_owned());
        ids
    }

    /// After a refresh or a filter change: every selection that still exists stays, an
    /// orchestrator no longer shown is parked (Python `SwarmApp.resolve`).
    pub(super) fn resolve(&mut self) {
        if let Some(o) = &self.view.orch
            && !self.shown_ids().contains(o)
        {
            self.parked = self.view.orch.take();
        }
        let cols = nav(&self.swarm, &self.view);
        if self
            .view
            .thread
            .as_ref()
            .is_some_and(|t| !cols[1].contains(t))
        {
            self.view.thread = None;
        }
        let cols = nav(&self.swarm, &self.view);
        let in_tree = |a: &String| cols[2..].iter().any(|c| c.contains(a));
        if self.view.agent.as_ref().is_some_and(|a| !in_tree(a)) {
            self.view.agent = None;
        }
    }

    /// The filter changes: a selection it hides is parked; the shown orchestrator stays
    /// selected while the filter shows it, and none is selected while it hides it.
    pub(super) fn apply_filter(&mut self, f: Filter) {
        self.view.filter = f;
        self.view.wheel.clear();
        self.resolve();
        if self.view.orch.is_none() && !self.back_to_shown() {
            self.view.focus = 0;
            if self.active.is_some() {
                self.parked = self.active.clone(); // the arrows resume from it
            }
        }
    }

    /// No selected row while the filter shows the orchestrator herdr shows: it is selected
    /// again. Whether that changed anything.
    pub(super) fn back_to_shown(&mut self) -> bool {
        let Some(active) = self.active.clone() else {
            return false;
        };
        if self.view.orch.is_some() || !self.shown_ids().contains(&active) {
            return false;
        }
        self.view.orch = Some(active);
        default_children(&self.swarm, &mut self.view);
        true
    }

    /// Where the arrows resume from while no row is selected.
    pub fn parked(&self) -> Option<&str> {
        self.parked.as_deref()
    }
}
