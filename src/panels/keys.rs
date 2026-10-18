//! The operator's side (Python `Manager.open_key`, `remove_all`): the open key in per-tab
//! mode, and closing every panel provably ours when per-tab goes off or the home quits.

use super::Manager;
use super::ops::Ops;
use super::snap::Snap;
use super::survey::HomeRef;
use panorama_core::model::Swarm;
use std::collections::BTreeSet;

/// What the open key did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenKey {
    /// Not in an orchestrator tab: the caller shows the home tab.
    Home,
    /// The tab's panel has the focus.
    Panel,
    /// No panel could be made or focused.
    Failed,
    /// The home lock stayed busy.
    Busy,
}

impl<O: Ops> Manager<O> {
    /// In an orchestrator tab with a panel, focus the panel; in one without (closed by hand, or
    /// not made yet), clear its closed mark, make it and focus it; anywhere else: home.
    pub async fn open_key(
        &mut self,
        snap: &Snap,
        swarm: &Swarm,
        home: Option<&HomeRef>,
        verified: &BTreeSet<String>,
        single: bool,
    ) -> OpenKey {
        let panel = {
            let Ok(_held) = self.lock().await else {
                return OpenKey::Busy;
            };
            let sv = self.survey(snap, swarm, home, single).await;
            let Some((tab, agent)) = sv
                .focused_tab
                .as_ref()
                .and_then(|t| Some((t.clone(), sv.targets.get(t)?.clone())))
            else {
                return OpenKey::Home;
            };
            if self.saved.closed.remove(&tab) {
                self.save();
            }
            match self.panels_map(&sv, verified).get(&tab).cloned() {
                Some(p) => p,
                None => match self.place(&tab, &agent, true).await {
                    Some(p) => p,
                    None => return OpenKey::Failed,
                },
            }
        };
        match self.ops.focus_pane(&panel).await {
            Ok(()) => OpenKey::Panel,
            Err(_) => OpenKey::Failed,
        }
    }

    /// Per-tab off, or the home panorama quits: close every panel provably ours (and empty the
    /// record: a clean end leaves nothing for a restore to bring back). How many were closed.
    pub async fn remove_all(&mut self, snap: Option<Snap>, verified: &BTreeSet<String>) -> usize {
        let snap = match snap {
            Some(s) => s,
            None => self.ops.snapshot().await.unwrap_or_default(),
        };
        let mut n = 0;
        for p in &snap.panes {
            if p.agent.as_deref().is_none_or(str::is_empty)
                && self.ours(&p.pane_id, verified)
                && self.is_panel(&p.pane_id).await == Some(true)
                && self.ops.close_plugin_pane(&p.pane_id).await.is_ok()
            {
                n += 1;
            }
        }
        self.created.clear();
        self.right.clear();
        if !self.saved.panes.is_empty() {
            self.saved.panes.clear();
            self.save();
        }
        n
    }
}
