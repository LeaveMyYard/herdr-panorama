//! The per-tab panels' manager, driven from the home view (Python `SwarmApp.panels_pass`,
//! `_panels_worker`, `_open_key`, and the panels' part of `action_quit`). One task owns the
//! manager, so passes never overlap: a pass asked for while one runs or waits is dropped (one
//! is enough, as Python's skipped worker). The open key and quit go first.

use super::done::Done;
use crate::core::Core;
use crate::home::config::{follow_tabs_single, per_tab};
use crate::home::label::is_home_label;
use crate::home::real::RealHome;
use crate::home::{HomeOps, Places, ensure, find_home};
use crate::panels::real::RealOps;
use crate::panels::{HomeRef, Manager, OpenKey, Ops};
use crate::settings::Settings;
use crate::ui::guard::guarded;
use herdr_agent_kit::herdr_api::PaneId;
use panorama_core::model::Swarm;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

/// How long quit gives the viewers to hear `shutdown`.
pub const SHUTDOWN_WAIT: Duration = Duration::from_secs(1);

enum Ask {
    /// The open key from the core: what it did.
    OpenKey(oneshot::Sender<String>),
    /// Quit: the viewers told to go, every panel provably ours closed.
    Quit(oneshot::Sender<()>),
}

/// The handle the home view keeps.
#[derive(Clone)]
pub struct PanelsLink {
    pass: mpsc::Sender<()>,
    ask: mpsc::UnboundedSender<Ask>,
}

/// What the task owns.
pub struct PanelsTask {
    pub manager: Manager<RealOps>,
    pub core: Arc<Core>,
    /// The last model; `None` until the first successful collect: no pass runs before it
    /// (Python's `not self.loaded`), so the start batch and the restore check see real tabs.
    pub swarm: Arc<Mutex<Option<Swarm>>>,
    pub settings: Settings,
    pub home: RealHome,
    pub places: Places,
    pub my_pane: Option<String>,
    pub done: mpsc::UnboundedSender<Done>,
    pub log: PathBuf,
    /// Tests: the next passes panic (a step's panic must never end the task).
    #[cfg(test)]
    pub explode: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl PanelsLink {
    /// Starts the task.
    pub fn spawn(task: PanelsTask) -> Self {
        let (pass, pass_rx) = mpsc::channel(1);
        let (ask, ask_rx) = mpsc::unbounded_channel();
        tokio::spawn(task.serve(pass_rx, ask_rx));
        Self { pass, ask }
    }

    /// A pass soon (none is added while one waits).
    pub fn pass(&self) {
        let _ = self.pass.try_send(());
    }

    /// The open key: what it did ("panel", "home", "failed", "busy").
    pub async fn open_key(&self) -> String {
        let (tx, rx) = oneshot::channel();
        if self.ask.send(Ask::OpenKey(tx)).is_err() {
            return "failed".into();
        }
        rx.await.unwrap_or_else(|_| "failed".into())
    }

    /// Quit: returns once the panels are closed (or the task is gone).
    pub async fn quit(&self) {
        let (tx, rx) = oneshot::channel();
        if self.ask.send(Ask::Quit(tx)).is_ok() {
            let _ = rx.await;
        }
    }
}

impl PanelsTask {
    async fn serve(mut self, mut pass: mpsc::Receiver<()>, mut ask: mpsc::UnboundedReceiver<Ask>) {
        loop {
            tokio::select! {
                biased;
                a = ask.recv() => match a {
                    Some(Ask::OpenKey(reply)) => {
                        let did = guarded(self.open_key()).await;
                        let did = self.caught("the open key", did).unwrap_or_else(|| "failed".into());
                        let _ = reply.send(did);
                    }
                    Some(Ask::Quit(reply)) => {
                        let done = guarded(self.quit()).await;
                        self.caught("quit", done);
                        let _ = reply.send(());
                        return;
                    }
                    None => return,
                },
                Some(()) = pass.recv() => {
                    let done = guarded(self.pass()).await;
                    self.caught("a pass", done); // the next pass tries again, as Python's worker
                }
            }
            for note in self.manager.take_notes() {
                crate::app::panic::log(&self.log, &format!("panels: {note}"));
            }
        }
    }

    /// A step that panicked is logged, never the task's end.
    fn caught<T>(&self, what: &str, r: Result<T, String>) -> Option<T> {
        r.map_err(|why| crate::app::panic::log(&self.log, &format!("panels: {what} failed: {why}")))
            .ok()
    }

    fn swarm(&self) -> Option<Swarm> {
        // a panic elsewhere while holding it leaves the model as it was: still the model
        self.swarm.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    async fn home_ref(&self) -> Option<HomeRef> {
        let h = find_home(&self.home, &self.places, self.my_pane.as_deref()).await?;
        Some(HomeRef {
            pane_id: h.pane_id,
            tab_id: h.tab_id,
        })
    }

    /// One pass; the first is the start batch. Per-tab switched off while running: the
    /// panels go, once.
    async fn pass(&mut self) {
        let cfg = self.settings.panorama_table();
        let verified = self.core.verified_panes();
        if !per_tab(&cfg) {
            if self.manager.started {
                self.core.shutdown(SHUTDOWN_WAIT).await; // the viewers exit; their panes close
                self.manager.remove_all(None, &verified).await;
                self.manager.started = false;
                let _ = self.done.send(Done::Panels(Default::default()));
            }
            return;
        }
        #[cfg(test)]
        if self.explode.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            self.explode
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            panic!("a pass that panics (test)");
        }
        let Some(swarm) = self.swarm() else {
            return; // no model yet
        };
        let Some(snap) = self.manager.ops.snapshot().await else {
            return;
        };
        let home = self.home_ref().await;
        let start = !self.manager.started;
        let single = follow_tabs_single(&cfg);
        if let Some(sv) = self
            .manager
            .reconcile(snap, &swarm, home.as_ref(), &verified, start, single)
            .await
        {
            let _ = self
                .done
                .send(Done::Panels(self.manager.panels_map(&sv, &verified)));
        }
    }

    /// The open key in per-tab mode: the manager decides; "home" shows the home view (moved
    /// back to a tab of its own first if an older version left it docked).
    async fn open_key(&mut self) -> String {
        let cfg = self.settings.panorama_table();
        if per_tab(&cfg) {
            let snap = self.manager.ops.snapshot().await.unwrap_or_default();
            let (home, swarm, verified) = (
                self.home_ref().await,
                self.swarm().unwrap_or_default(), // before the first model: as empty, as Python
                self.core.verified_panes(),
            );
            let did = self
                .manager
                .open_key(
                    &snap,
                    &swarm,
                    home.as_ref(),
                    &verified,
                    follow_tabs_single(&cfg),
                )
                .await;
            match did {
                OpenKey::Panel => return "panel".into(),
                OpenKey::Failed => return "failed".into(),
                OpenKey::Busy => return "busy".into(),
                OpenKey::Home => {}
            }
        }
        self.show_home().await
    }

    async fn show_home(&mut self) -> String {
        if self.my_pane.is_none() {
            return "home".into(); // no pane of its own to show (Python: `and self.my_pane`)
        }
        let Some(mut me) = find_home(&self.home, &self.places, self.my_pane.as_deref()).await
        else {
            return "failed".into();
        };
        let label = self
            .home
            .snapshot()
            .await
            .map(|s| s.tab_label(&me.tab_id).to_owned())
            .unwrap_or_default();
        if !is_home_label(&label) {
            match ensure(&self.home, &self.places, true).await {
                Ok(moved) => {
                    self.my_pane = Some(moved.pane_id.clone());
                    let _ = self.done.send(Done::MyPane(moved.pane_id.clone()));
                    me = moved;
                }
                Err(_) => return "failed".into(),
            }
        }
        let shown = self.home.focus_tab(&me.tab_id).await.is_ok()
            && self
                .home
                .herdr
                .focus_pane(&PaneId::new(&me.pane_id))
                .await
                .is_ok();
        if shown { "home" } else { "failed" }.into()
    }

    /// Quit: the viewers are told to go (their panes close with them), then every panel
    /// provably ours is closed (Python's order).
    async fn quit(&mut self) {
        self.core.shutdown(SHUTDOWN_WAIT).await;
        let verified = self.core.verified_panes();
        self.manager.remove_all(None, &verified).await;
    }
}
