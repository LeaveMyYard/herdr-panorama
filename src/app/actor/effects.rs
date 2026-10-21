//! What the input asks for, carried out (Python `SwarmApp`'s actions' IO): a notice and the
//! end here; going to a pane, the inbox, + New agent and zoom on the herdr side
//! ([`crate::app::link`], off the actor). Without a link (tests) those do nothing. Agents seen
//! go to the producer's unseen tracker ([`crate::app::seen`]).

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
                // the marks cleared in the view; the producer's tracker keeps them cleared
                Effect::Seen(ids) => {
                    if let Some(t) = self.worker.tracker() {
                        crate::app::seen::mark(&t, &ids);
                    }
                }
                Effect::Goto { pane, .. } => self.on_link_ref(|l| l.goto(pane)),
                Effect::GotoRemote {
                    machine_id,
                    machine,
                    pane,
                    label,
                } => {
                    self.on_link_ref(|l| l.goto_remote(machine_id, machine, pane, label));
                }
                Effect::OpenInbox { question, pane } => {
                    self.on_link_ref(|l| l.open_inbox(question, pane))
                }
                Effect::NewAgent { cwd } => {
                    let width = self.driver.app.width;
                    self.on_link_ref(|l| l.new_agent(cwd, width));
                }
                Effect::Zoom => self.on_link_ref(|l| l.zoom()),
                // the panel's own (a core, the docked list's arrows): the app never asks
                Effect::SendFilter(_) | Effect::Switch { .. } | Effect::ArrowsRestAt(_) => {}
            }
        }
        None
    }

    fn on_link_ref(&self, f: impl FnOnce(&crate::app::link::Link)) {
        if let Some(l) = &self.link {
            f(l);
        }
    }
}
