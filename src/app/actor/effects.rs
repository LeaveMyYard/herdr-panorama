//! What the input asks for, carried out (Python `SwarmApp`'s actions' IO): a notice, the end.
//! The herdr side (going to a pane, the inbox, a new agent, zoom) and the unseen tracker
//! join in P4 8c; until then those effects do nothing here.

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
                // P4 8c: the herdr side and the unseen tracker
                Effect::Seen(_)
                | Effect::Goto { .. }
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
