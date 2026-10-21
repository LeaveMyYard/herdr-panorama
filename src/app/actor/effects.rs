//! What the input asks for, carried out (Python `SwarmApp`'s actions' IO): a notice, the end.
//! Agents seen go to the producer's unseen tracker ([`crate::app::seen`]). The herdr side
//! (going to a pane, the inbox, a new agent, zoom) joins in P4 8c; until then it does nothing.

use super::{Actor, End};
use crate::ui::input::Effect;
use std::time::Instant;

impl Actor {
    /// Carries out `effects`; `Some` when one ends the app.
    pub(super) fn act(&mut self, effects: Vec<Effect>) -> Option<End> {
        for e in effects {
            match e {
                Effect::Quit => return Some(End::Quit),
                Effect::Toast(t) => self.driver.toasts.push(t, Instant::now()),
                Effect::Seen(ids) => {
                    if let Some(t) = self.worker.tracker() {
                        crate::app::seen::mark(&t, &ids);
                    }
                }
                // P4 8c: the herdr side
                Effect::Goto { .. }
                | Effect::GotoRemote { .. }
                | Effect::OpenInbox { .. }
                | Effect::NewAgent { .. }
                | Effect::Zoom => {}
                // the panel's own (a core, the docked list's arrows): the app never asks
                Effect::SendFilter(_) | Effect::Switch { .. } | Effect::ArrowsRestAt(_) => {}
            }
        }
        None
    }
}
