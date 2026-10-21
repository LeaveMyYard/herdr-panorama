//! What the panel supplies to the shared input (Python `PanelApp`'s overrides of
//! `SwarmApp`): its arming and click arming, the docked list, Enter going to panes, the arrows
//! switching the chats, the core it needs, and its own going to a pane, new agent and filter.

use super::effect::Effect;
use crate::panel::model::Panel;
use crate::ui::input::host::Host;
use panorama_core::view::Filter;
use std::time::Instant;

impl Host for Panel {
    fn armed(&self, now: Instant) -> bool {
        Panel::armed(self, now)
    }

    fn click_ready(&self, cell: (i32, i32), now: Instant) -> bool {
        self.input.clicks.ready(cell, now)
    }

    fn dock(&self) -> bool {
        self.input.dock
    }

    fn enter_goes_to_pane(&self) -> bool {
        true // a panel is in the home view's place: Enter on an orchestrator goes to its pane
    }

    fn arrows_mode(&self) -> bool {
        Panel::arrows_mode(self)
    }

    fn usable(&self, out: &mut Vec<Effect>) -> bool {
        Panel::usable(self, out)
    }

    fn goto(&mut self, switch: bool, now: Instant) -> Vec<Effect> {
        Panel::goto(self, switch, now)
    }

    fn new_agent(&mut self) -> Vec<Effect> {
        Panel::new_agent(self)
    }

    fn set_filter(&mut self, f: Filter, now: Instant) -> Vec<Effect> {
        Panel::set_filter(self, f, now)
    }
}
