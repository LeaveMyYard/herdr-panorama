//! Carrying out the input's effects (Python's workers and notices): the core hears seen
//! marks and filters at once (a filter send is bounded, so a core that stopped reading never
//! holds the keys); switches, pane checks, the inbox and the + New agent popup run as their
//! own guarded tasks (never blocking the actor, a panic in one reported as its failure) and
//! report back as [`Done`].

use super::announce::Announcer;
use super::guard::guarded;
use super::{Actor, End};
use crate::panel::act::inbox::UvRunner;
use crate::panel::act::switch::{Switched, switch};
use crate::panel::act::{goto, new_agent, remote};
use crate::panel::input::{Effect, Painted, Toast};
use std::future::Future;
use std::io;
use std::time::{Duration, Instant};

/// How long the actor waits for a filter's send (keys and paint go on after it).
pub const FILTER_SEND: Duration = Duration::from_millis(500);

/// What a task of the herdr side reports back.
#[derive(Debug)]
pub enum Done {
    /// A switch ended.
    Switched(Switched),
    /// A pane checked before going to it; `nav`: the navigation it belongs to.
    Checked {
        pane: String,
        label: String,
        keep_focus: bool,
        seen: Vec<String>,
        exists: bool,
        nav: u64,
    },
    /// A notice to show.
    Toast(Toast),
}

/// Whether the filter went out: `Some(false)` when the client said there is no core;
/// `None` when it did not answer within `within` (the client has queued it and still sends
/// it, so it is not taken back).
pub async fn bounded(send: impl Future<Output = bool>, within: Duration) -> Option<bool> {
    tokio::time::timeout(within, send).await.ok()
}

/// The notice for a task that panicked.
fn failed(what: &str, why: &str) -> Done {
    Done::Toast(Toast::new(Some("panorama"), format!("{what} failed: {why}"), 8).warning())
}

impl Actor {
    /// Carries out `effects`; `Some` when the panel ends.
    pub(super) async fn act(&mut self, effects: Vec<Effect>) -> io::Result<Option<End>> {
        for e in effects {
            match e {
                Effect::Seen(ids) => self.client.seen(ids),
                Effect::SendFilter(f) => {
                    if bounded(self.client.filter(f.name()), FILTER_SEND).await == Some(false) {
                        self.driver.panel.filter_not_sent();
                    }
                }
                Effect::Toast(t) => self.driver.toasts.push(t, Instant::now()),
                Effect::ArrowsRestAt(at) => self.arrow_at = Some(at),
                Effect::Quit => return Ok(Some(End::Quit)),
                Effect::Switch { pane, keep_focus } => self.spawn_switch(pane, keep_focus),
                Effect::Goto {
                    pane,
                    label,
                    keep_focus,
                    seen,
                } => {
                    let nav = self.driver.launched();
                    let h = self.herdr.clone();
                    self.spawn("going to the pane", async move {
                        let exists = goto::exists(&h, &pane).await;
                        Some(Done::Checked {
                            pane,
                            label,
                            keep_focus,
                            seen,
                            exists,
                            nav,
                        })
                    });
                }
                Effect::GotoRemote {
                    machine_id,
                    machine,
                    pane,
                    label,
                } => {
                    let h = self.herdr.clone();
                    self.spawn("going to the remote pane", async move {
                        let text =
                            remote::goto_remote(&h, &machine_id, &machine, &pane, &label).await;
                        Some(Done::Toast(Toast::new(Some(remote::TITLE), text, 8)))
                    });
                }
                Effect::OpenInbox { question, pane } => {
                    let (h, inbox) = (self.herdr.clone(), self.inbox.clone());
                    self.spawn("opening the inbox", async move {
                        // Python ignores how the inbox opened
                        inbox.open(&h, &UvRunner, question, pane.as_deref()).await;
                        None
                    });
                }
                Effect::NewAgent { cwd } => {
                    let (h, place) = (self.herdr.clone(), self.place.clone());
                    self.spawn("+ New agent", async move {
                        let cwd = cwd.unwrap_or(place.home.clone());
                        let root = place.root.as_deref();
                        let failed = new_agent::open(
                            &h,
                            &place.plugin_id,
                            root,
                            &cwd,
                            &place.new_agent_mark,
                        )
                        .await;
                        failed.map(|text| {
                            Done::Toast(Toast::new(Some("new agent"), text, 6).warning())
                        })
                    });
                }
            }
        }
        Ok(None)
    }

    fn spawn_switch(&mut self, pane: String, keep_focus: bool) {
        self.driver.launched();
        let (h, tx, panes_of) = (
            self.herdr.clone(),
            self.done_tx.clone(),
            self.driver.panel.panes_of.clone(),
        );
        let mine = self.driver.panel.my_pane.clone();
        let announcer = Announcer::new(self.client.clone(), self.focus.subscribe());
        tokio::spawn(async move {
            let run = guarded(async move {
                switch(
                    &h,
                    &pane,
                    keep_focus,
                    &panes_of,
                    mine.as_deref(),
                    &announcer,
                )
                .await
            });
            // a switch that panicked still ends, as a failure (Python's worker caught all)
            let done = run.await.unwrap_or_else(|why| Switched {
                err: Some(format!("switch failed: {why}")),
                took_back: false,
            });
            let _ = tx.send(Done::Switched(done));
        });
    }

    /// Runs `task` as its own guarded task; its report, or its panic as a notice, comes back.
    fn spawn(&self, what: &'static str, task: impl Future<Output = Option<Done>> + Send + 'static) {
        let tx = self.done_tx.clone();
        tokio::spawn(async move {
            let report = guarded(task)
                .await
                .unwrap_or_else(|why| Some(failed(what, &why)));
            if let Some(r) = report {
                let _ = tx.send(r);
            }
        });
    }

    /// A report from the herdr side; `Some` when the panel ends.
    pub(super) async fn on_done(&mut self, done: Done) -> io::Result<Option<End>> {
        let effects = self.driver.done(done, Instant::now());
        if let Some(end) = self.act(effects).await? {
            return Ok(Some(end));
        }
        self.repaint(Painted::Other).map(|()| None)
    }
}

#[cfg(test)]
mod tests;
