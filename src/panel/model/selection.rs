//! The panel's selected row (Python `PanelApp._herdr_shows`, `_apply_filter`,
//! `_back_to_shown`, `_filter_to`), over the selection rules both views share
//! ([`crate::ui::selection`]).

use super::Panel;
use crate::ui::layout;
use crate::ui::selection::{self, SelectedCard};
use panorama_core::model::Swarm;
use panorama_core::view::Filter;
use panorama_core::view::nav::default_children;

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
    /// The filter changes: a selection it hides is parked; the shown orchestrator stays
    /// selected while the filter shows it, and none is selected while it hides it.
    pub(crate) fn apply_filter(&mut self, f: Filter) {
        self.graph.view.filter = f;
        self.graph.view.wheel.clear();
        self.resolve();
        if self.graph.view.orch.is_none() && !self.back_to_shown() {
            self.graph.view.focus = 0;
            if self.active.is_some() {
                self.graph.parked = self.active.clone(); // the arrows resume from it
            }
        }
    }

    /// No selected row while the filter shows the orchestrator herdr shows: it is selected
    /// again. Whether that changed anything.
    pub(super) fn back_to_shown(&mut self) -> bool {
        let Some(active) = self.active.clone() else {
            return false;
        };
        if self.graph.view.orch.is_some() || !self.shown_ids().contains(&active) {
            return false;
        }
        self.graph.view.orch = Some(active);
        default_children(&self.graph.swarm, &mut self.graph.view);
        true
    }

}

impl Panel {
    /// Docked or narrow (Python `_update_dock`): docked while this tab has an orchestrator
    /// and the pane (as last painted) is narrower than the wide layout; the docked list is
    /// one column. Decided on each refresh before the arming comparison, and on each paint.
    pub(crate) fn update_dock(&mut self) {
        self.input.dock = layout::docked(&self.graph.swarm, self.input.width);
        if self.input.dock {
            self.graph.view.focus = 0;
        }
    }

    /// What an action key would act on (a refresh that changes it re-arms the keys).
    pub(crate) fn selected_card(&self) -> SelectedCard {
        selection::selected_card(&self.graph.swarm, &self.graph.view, self.input.dock)
    }
}
