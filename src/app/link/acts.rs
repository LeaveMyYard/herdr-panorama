//! The input's herdr side for the app (Python `SwarmApp.action_goto`, `_goto_remote`,
//! `_open_inbox`, `action_new_agent`, `action_zoom`): each runs as its own guarded task and
//! reports back as [`Done`]; a panic in one is its failure, never the view's.

use super::Link;
use super::done::Done;
use crate::panel::act::inbox::UvRunner;
use crate::panel::act::{new_agent, remote};
use crate::ui::guard::guarded;
use crate::ui::input::Toast;
use herdr_agent_kit::herdr_api::{PaneId, clean};
use std::future::Future;

/// The + New agent form is 76 columns wide (plus its margin): narrower home panes use a popup
/// (Python `NEW_AGENT_MIN_WIDTH`).
pub const NEW_AGENT_MIN_WIDTH: i32 = 80;

impl Link {
    /// Runs `fut` off the actor; what it reports goes back as [`Done`]. A panic is reported as
    /// a notice that `what` failed.
    pub(super) fn spawn(
        &self,
        what: &'static str,
        fut: impl Future<Output = Option<Done>> + Send + 'static,
    ) {
        let tx = self.done_tx.clone();
        tokio::spawn(async move {
            let done = match guarded(fut).await {
                Ok(d) => d,
                Err(why) => Some(Done::Toast(
                    Toast::new(Some("panorama"), format!("{what} failed: {why}"), 8).warning(),
                )),
            };
            if let Some(d) = done {
                let _ = tx.send(d);
            }
        });
    }

    /// g / Enter: into the orchestrator's agent. A popup closes once it is there; the home
    /// view stays (Python: `elif not self.home_mode: self.exit()`).
    pub fn goto(&self, pane: String) {
        let (h, home) = (self.herdr.clone(), self.home_mode);
        self.spawn("going to the pane", async move {
            match h.focus_agent(&PaneId::new(&pane)).await {
                Ok(_) if home => None,
                Ok(_) => Some(Done::Exit),
                Err(e) => Some(Done::Toast(
                    Toast::new(
                        None,
                        format!("could not focus {pane}: {}", clean(&e.to_string(), 200)),
                        5,
                    )
                    .warning(),
                )),
            }
        });
    }

    /// An orchestrator on another machine: focused on its own server, and where it is said.
    pub fn goto_remote(&self, machine_id: String, machine: String, pane: String, label: String) {
        let h = self.herdr.clone();
        self.spawn("going to the remote pane", async move {
            let text = remote::goto_remote(&h, &machine_id, &machine, &pane, &label).await;
            Some(Done::Toast(Toast::new(Some(remote::TITLE), text, 8)))
        });
    }

    /// herdr-inbox on a question's group, a pane's, or plain. The home tab is not modal: the
    /// inbox popup opens over it; a popup closes first.
    pub fn open_inbox(&self, question: Option<i64>, pane: Option<String>) {
        if !self.home_mode {
            let _ = self.done_tx.send(Done::ExitThenInbox { question, pane });
            return;
        }
        let (h, inbox) = (self.herdr.clone(), self.inbox.clone());
        self.spawn("opening the inbox", async move {
            inbox.open(&h, &UvRunner, question, pane.as_deref()).await; // Python ignores how it went
            None
        });
    }

    /// The + New agent form in this pane where it fits, else (a narrow home pane) the
    /// plugin's own popup; a popup herdr refused falls back to the form.
    pub fn new_agent(&self, cwd: Option<String>, width: i32) {
        let cwd = cwd.unwrap_or_else(|| self.place.home.clone());
        if !(self.home_mode && 0 < width && width < NEW_AGENT_MIN_WIDTH) {
            let _ = self.done_tx.send(Done::NewAgentForm(cwd));
            return;
        }
        let (h, place) = (self.herdr.clone(), self.place.clone());
        self.spawn("+ New agent", async move {
            let failed = new_agent::open(
                &h,
                &place.plugin_id,
                place.root.as_deref(),
                &cwd,
                &place.new_agent_mark,
            )
            .await;
            match failed {
                None => None,
                // no reply and no popup started: say so (n can be pressed again)
                Some(text) if text == new_agent::DID_NOT_OPEN => Some(Done::Toast(
                    Toast::new(Some("new agent"), text, 6).warning(),
                )),
                Some(_) => Some(Done::NewAgentForm(cwd)), // refused: the form in this pane
            }
        });
    }

    /// z: panorama's own pane full-screen in its tab, and back.
    pub fn zoom(&self) {
        let (h, Some(me)) = (self.herdr.clone(), self.my_pane.clone()) else {
            return;
        };
        if !self.home_mode {
            return;
        }
        self.spawn("zoom", async move {
            h.toggle_zoom(&PaneId::new(&me)).await.err().map(|e| {
                Done::Toast(Toast::new(Some("zoom"), clean(&e.to_string(), 200), 5).warning())
            })
        });
    }
}
