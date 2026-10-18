//! Carrying out the input's effects (Python's workers and notices): the core hears seen
//! marks and filters at once; switches, pane checks, the inbox and the + New agent popup run
//! as their own tasks (never blocking the actor) and report back as [`Done`].

use super::announce::Announcer;
use super::{Actor, End};
use crate::panel::act::inbox::UvRunner;
use crate::panel::act::switch::{Switched, switch};
use crate::panel::act::{goto, new_agent, remote};
use crate::panel::input::{Effect, Painted, Toast};
use std::io;
use std::time::Instant;

/// What a task of the herdr side reports back.
#[derive(Debug)]
pub enum Done {
    /// A switch ended.
    Switched(Switched),
    /// A pane checked before going to it.
    Checked {
        pane: String,
        label: String,
        keep_focus: bool,
        seen: Vec<String>,
        exists: bool,
    },
    /// A notice to show.
    Toast(Toast),
}

impl Actor {
    /// Carries out `effects`; `Some` when the panel ends.
    pub(super) async fn act(&mut self, effects: Vec<Effect>) -> io::Result<Option<End>> {
        for e in effects {
            match e {
                Effect::Seen(ids) => self.client.seen(ids),
                Effect::SendFilter(f) => {
                    if !self.client.filter(f.name()).await {
                        self.panel.filter_not_sent();
                    }
                }
                Effect::Toast(t) => self.toasts.push(t, Instant::now()),
                Effect::ArrowsRestAt(at) => self.arrow_at = Some(at),
                Effect::Quit => return Ok(Some(End::Quit)),
                Effect::Switch { pane, keep_focus } => self.spawn_switch(pane, keep_focus),
                other => self.spawn(other),
            }
        }
        Ok(None)
    }

    fn spawn_switch(&self, pane: String, keep_focus: bool) {
        let (h, tx, panes_of) = (
            self.herdr.clone(),
            self.done_tx.clone(),
            self.panel.panes_of.clone(),
        );
        let mine = self.panel.my_pane.clone();
        let announcer = Announcer::new(self.client.clone(), self.focus.subscribe());
        tokio::spawn(async move {
            let done = switch(
                &h,
                &pane,
                keep_focus,
                &panes_of,
                mine.as_deref(),
                &announcer,
            )
            .await;
            let _ = tx.send(Done::Switched(done));
        });
    }

    /// The other herdr-side effects, each as its own task.
    fn spawn(&self, e: Effect) {
        let (h, tx, place, inbox) = (
            self.herdr.clone(),
            self.done_tx.clone(),
            self.place.clone(),
            self.inbox.clone(),
        );
        tokio::spawn(async move {
            let report = match e {
                Effect::Goto {
                    pane,
                    label,
                    keep_focus,
                    seen,
                } => {
                    let exists = goto::exists(&h, &pane).await;
                    Some(Done::Checked {
                        pane,
                        label,
                        keep_focus,
                        seen,
                        exists,
                    })
                }
                Effect::GotoRemote {
                    machine_id,
                    machine,
                    pane,
                    label,
                } => {
                    let text = remote::goto_remote(&h, &machine_id, &machine, &pane, &label).await;
                    Some(Done::Toast(Toast::new(Some(remote::TITLE), text, 8)))
                }
                // Python ignores how the inbox opened
                Effect::OpenInbox { question, pane } => {
                    inbox.open(&h, &UvRunner, question, pane.as_deref()).await;
                    None
                }
                Effect::NewAgent { cwd } => {
                    let cwd = cwd.unwrap_or(place.home.clone());
                    let failed = new_agent::open(
                        &h,
                        &place.plugin_id,
                        place.root.as_deref(),
                        &cwd,
                        &place.new_agent_mark,
                    )
                    .await;
                    failed.map(|text| Done::Toast(Toast::new(Some("new agent"), text, 6).warning()))
                }
                _ => None,
            };
            if let Some(r) = report {
                let _ = tx.send(r);
            }
        });
    }

    /// A report from the herdr side; `Some` when the panel ends.
    pub(super) async fn on_done(&mut self, done: Done) -> io::Result<Option<End>> {
        let now = Instant::now();
        let effects = match done {
            Done::Switched(s) => self.panel.switched(s.err, s.took_back, now),
            Done::Checked {
                label,
                exists: false,
                ..
            } => vec![Effect::Toast(Toast::new(None, goto::gone(&label), 3))],
            Done::Checked {
                pane,
                keep_focus,
                seen,
                ..
            } => {
                let mut out = self.panel.seen_now(seen);
                out.extend(self.panel.switch(pane, keep_focus));
                out
            }
            Done::Toast(t) => vec![Effect::Toast(t)],
        };
        if let Some(end) = self.act(effects).await? {
            return Ok(Some(end));
        }
        self.repaint(Painted::Other).map(|()| None)
    }
}
