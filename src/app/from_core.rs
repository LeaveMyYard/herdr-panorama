//! What the per-tab panels tell the home view through the core (Python
//! `SwarmApp._filter_from_panel`): a panel's filter is taken here, and the state that goes out
//! with it tells every other panel.

use super::model::App;
use crate::ui::input::{Effect, Host};
use panorama_core::view::Filter;
use std::time::Instant;

impl App {
    /// A panel chose the filter `name`: taken when it names one (the effects of applying it;
    /// none when it is the shown one already); `None` for a name that is no filter, which is
    /// ignored and not published.
    pub fn filter_from_core(&mut self, name: &str, now: Instant) -> Option<Vec<Effect>> {
        let f = Filter::from_name(name)?;
        Some(if f == self.view.filter {
            vec![]
        } else {
            self.set_filter(f, now)
        })
    }
}
