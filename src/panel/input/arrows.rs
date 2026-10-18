//! The docked list's arrows (Python `_arrows_mode`, `_arrow_dock_later`, `_arrow_dock`,
//! `_docked`): with `dock_on_arrows`, ↑/↓ switch the chats themselves once they rest
//! ([`super::ARROW_REST`]), the keyboard staying on the panels; Enter goes into the agent.

use super::effect::{Effect, Toast};
use super::{ARROW_REST, InputState};
use crate::panel::model::Panel;
use panorama_core::view::NEW_ROW;
use panorama_core::view::nav::find_orch;
use std::time::Instant;

impl InputState {
    /// The arrows will rest at `now + ARROW_REST` (a new arrow moves it on).
    fn arrows_later(&mut self, now: Instant) -> Effect {
        let due = now + ARROW_REST;
        self.arrow_due = Some(due);
        Effect::ArrowsRestAt(due)
    }
}

impl Panel {
    /// The docked list with `dock_on_arrows`: the arrows switch the chats.
    pub(crate) fn arrows_mode(&self) -> bool {
        let s = self.input.settings;
        self.input.dock && s.arrows && s.keep_focus
    }

    /// ↑/↓.
    pub fn arrow(&mut self, delta: i32, now: Instant) -> Vec<Effect> {
        let moved_orchs = self.move_cursor(delta);
        if moved_orchs && self.arrows_mode() {
            vec![self.input.arrows_later(now)]
        } else {
            vec![]
        }
    }

    /// The arrows' rest came due at `now` (a later arrow set a later one: nothing yet). Switch
    /// to the orchestrator they rest on, the keyboard staying on the panels: not the docked
    /// one, not + New agent, not one on another machine, and only while the keyboard is
    /// still here.
    pub fn arrows_rested(&mut self, now: Instant) -> Vec<Effect> {
        if self.input.arrow_due.is_none_or(|due| now < due) {
            return vec![];
        }
        self.input.arrow_due = None;
        if !self.arrows_mode() || self.view.focus != 0 {
            return vec![];
        }
        if self.input.docking {
            self.input.arrow_again = true; // look again when this switch ends
            return vec![];
        }
        let elsewhere = self
            .herdr_focused_pane
            .as_ref()
            .is_some_and(|f| self.my_pane.as_ref().is_some_and(|m| m != f));
        if !self.pane_focused || elsewhere {
            return vec![];
        }
        let o = find_orch(
            &self.swarm,
            self.view.orch.as_deref().filter(|o| *o != NEW_ROW),
        );
        let Some(o) =
            o.filter(|o| o.machine.is_none() && self.swarm.docked.as_deref() != Some(&o.pane_id))
        else {
            return vec![];
        };
        let (pane, ids): (String, Vec<String>) = (
            o.pane_id.clone(),
            o.agents().map(|a| a.id.clone()).collect(),
        );
        let mut out = vec![];
        self.seen(ids, &mut out); // switched next to it: all of it is in view
        out.extend(self.switch(pane, true));
        out
    }

    /// A switch (Python `PanelApp._dock`), one at a time and only with a core.
    pub fn switch(&mut self, pane: String, keep_focus: bool) -> Vec<Effect> {
        if self.input.docking || !self.connected {
            return vec![];
        }
        self.input.docking = true;
        vec![Effect::Switch { pane, keep_focus }]
    }

    /// The switch ended (`err`: why it failed; `took_back`: it announced this panel again, so
    /// this panel is the focused one): the focus is judged again, arrows that came to rest
    /// meanwhile are looked at again, and a failure is said.
    pub fn switched(&mut self, err: Option<String>, took_back: bool, now: Instant) -> Vec<Effect> {
        self.input.docking = false;
        if took_back {
            self.herdr_focused_pane = self.my_pane.clone();
        }
        if let Some(f) = self.herdr_focused_pane.clone() {
            self.herdr_focus(&f);
        }
        let mut out = vec![];
        if std::mem::take(&mut self.input.arrow_again) {
            out.push(self.input.arrows_later(now));
        }
        if let Some(err) = err {
            let text = herdr_agent_kit::herdr_api::clean(&err, 300);
            out.push(Effect::Toast(
                Toast::new(Some("cockpit"), text, 8).warning(),
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests;
