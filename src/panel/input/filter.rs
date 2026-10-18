//! The filter chosen in this panel (Python `action_filter`, `PanelApp._set_filter`): shown
//! here at once and sent to the core for every other view.

use super::effect::Effect;
use crate::panel::model::Panel;
use panorama_core::view::Filter;
use std::time::Instant;

impl Panel {
    /// f: the next filter, shown here at once and sent to the core for every other view.
    pub fn next_filter(&mut self, now: Instant) -> Vec<Effect> {
        let i = Filter::ALL
            .iter()
            .position(|f| *f == self.graph.view.filter)
            .unwrap_or(0);
        self.set_filter(Filter::ALL[(i + 1) % Filter::ALL.len()], now)
    }

    /// The operator chose `f` here (f, or a click on a filter): shown at once, and sent to the
    /// core for every other view; kept against the core's older word until it echoes.
    pub(crate) fn set_filter(&mut self, f: Filter, now: Instant) -> Vec<Effect> {
        self.apply_filter(f);
        self.input.filter_sent = Some((f, now));
        vec![Effect::SendFilter(f)]
    }

    /// The filter did not go out (no core): the next core's state decides.
    pub fn filter_not_sent(&mut self) {
        self.input.filter_sent = None;
    }
}
