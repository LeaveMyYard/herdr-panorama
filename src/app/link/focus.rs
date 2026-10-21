//! The state the panels hear and the focus they show (Python `SwarmApp.publish`,
//! `herdr_focus`, `focusing`, `_focus_read`, `_tab_shown`).

use super::state::state_message;
use super::{Done, FOCUS_CONFIRM, Link, LinkEvent};
use panorama_core::model::Swarm;
use panorama_core::view::{Filter, Marks};

impl Link {
    /// A refresh: the panels hear the new model, and a panels pass follows.
    pub fn refreshed(&mut self, swarm: &Swarm, unseen: &Marks, filter: Filter) {
        *self.swarm.lock().unwrap_or_else(|e| e.into_inner()) = Some(swarm.clone());
        self.publish(swarm, unseen, filter);
        if let Some(p) = &self.panels {
            p.pass();
        }
    }

    /// The whole state to the panel viewers (nothing without a core).
    pub fn publish(&mut self, swarm: &Swarm, unseen: &Marks, filter: Filter) {
        let Some(core) = &self.core else { return };
        self.published_filter = Some(filter);
        core.publish(&state_message(
            swarm,
            unseen,
            &self.published,
            Some(filter.name()),
        ));
    }

    /// Whether `filter` differs from what the panels last heard.
    pub fn filter_changed(&self, filter: Filter) -> bool {
        self.core.is_some() && self.published_filter != Some(filter)
    }

    /// A report from the herdr side, for the actor: the home pane's new id is taken here
    /// first (the open key moved it; the focus and the collector use it from now on).
    pub(super) fn reported(&mut self, d: Done) -> LinkEvent {
        if let Done::MyPane(pane) = &d {
            self.moved(pane.clone());
        }
        LinkEvent::Done(d)
    }

    /// herdr says `pane` has the focus: the view is focused exactly when it is its own pane;
    /// `Some` with the new look when that is known.
    pub fn herdr_focus(&mut self, pane: &str) -> Option<bool> {
        self.published.focused_pane = Some(pane.to_owned());
        self.unconfirmed = None; // any focus herdr reports settles an announced one
        let me = self.my_pane.as_deref().filter(|m| !m.is_empty())?;
        (!pane.is_empty()).then(|| pane == me)
    }

    /// A panel is about to put the keyboard on `pane`: shown at once, provisionally; herdr's
    /// real focus is read if no focus event confirms it in time.
    pub fn focusing(&mut self, pane: &str) -> Option<bool> {
        let look = self.herdr_focus(pane);
        self.unconfirmed = Some(pane.to_owned());
        let (h, announced) = (self.herdr.clone(), pane.to_owned());
        self.spawn("reading the focus", async move {
            tokio::time::sleep(FOCUS_CONFIRM).await;
            let real = h
                .snapshot()
                .await
                .ok()
                .and_then(|s| s.focused_pane_id)
                .map(|p| p.as_str().to_owned());
            Some(Done::FocusRead { announced, real })
        });
        look
    }

    /// The real focus read after an announcement: taken only if nothing settled it meanwhile.
    pub fn focus_read(&mut self, announced: &str, real: Option<String>) -> Option<bool> {
        if self.unconfirmed.as_deref() != Some(announced) {
            return None;
        }
        self.herdr_focus(&real.filter(|r| !r.is_empty())?)
    }

    /// herdr shows `tab`: whether that changed (then the state goes out again).
    pub fn tab_shown(&mut self, tab: String) -> bool {
        let changed = self.published.visible_tab.as_ref() != Some(&tab);
        self.published.visible_tab = Some(tab);
        changed
    }
}
