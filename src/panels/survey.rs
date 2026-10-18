//! One look at herdr (Python `Manager.survey` and `is_panel`): the tabs that should have a
//! panel and the panels there are.

use super::Manager;
use super::ops::Ops;
use super::snap::Snap;
use panorama_core::model::Swarm;
use std::collections::{BTreeMap, BTreeSet};

/// Where panorama's own home pane is (the home module finds it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HomeRef {
    pub pane_id: String,
    pub tab_id: String,
}

/// One look at herdr.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Survey {
    /// tab → its orchestrator's pane.
    pub targets: BTreeMap<String, String>,
    /// tab → the panel panes there (runs our viewer), in herdr's order.
    pub panels: BTreeMap<String, Vec<String>>,
    /// Tabs with a pane herdr could not check.
    pub unknown: BTreeSet<String>,
    /// Every tab in the snapshot.
    pub tabs: BTreeSet<String>,
    pub focused_tab: Option<String>,
    pub focused_pane: Option<String>,
    pub home_tab: Option<String>,
}

impl<O: Ops> Manager<O> {
    /// Whether the pane's foreground runs `panorama panel` (`None`: herdr could not say). An
    /// answer is believed for a while, either way.
    pub async fn is_panel(&mut self, pane: &str) -> Option<bool> {
        if self.created.contains_key(pane) {
            return Some(true);
        }
        let now = (self.clock)();
        if self.not_panel.get(pane).is_some_and(|until| *until > now) {
            return Some(false);
        }
        if self.panel.get(pane).is_some_and(|until| *until > now) {
            return Some(true);
        }
        let runs = self.ops.runs_panel(pane).await;
        match runs {
            Some(false) => {
                _ = self
                    .not_panel
                    .insert(pane.to_owned(), now + super::NOT_PANEL)
            }
            Some(true) => _ = self.panel.insert(pane.to_owned(), now + super::PANEL),
            None => {}
        }
        runs
    }

    /// Which tabs should have a panel (the model's local orchestrators, checked against the
    /// snapshot: a claude pane, not in the home tab, in the home pane's workspace) and which
    /// panels exist. `single`: `cockpit_follow_tabs = "single"`, only tabs whose one pane is
    /// the orchestrator.
    pub async fn survey(
        &mut self,
        snap: &Snap,
        swarm: &Swarm,
        home: Option<&HomeRef>,
        single: bool,
    ) -> Survey {
        let mut sv = Survey {
            focused_tab: snap.focused_tab.clone(),
            focused_pane: snap.focused_pane.clone(),
            home_tab: home.map(|h| h.tab_id.clone()),
            ..Survey::default()
        };
        sv.tabs = snap.panes.iter().filter_map(|p| p.tab_id.clone()).collect();
        let workspace = home
            .and_then(|h| snap.pane(&h.pane_id))
            .and_then(|p| p.workspace_id.clone());
        for o in swarm.orchestrators.iter().filter(|o| o.machine.is_none()) {
            let Some(p) = snap.pane(&o.pane_id) else {
                continue;
            };
            let Some(tab) = p
                .tab_id
                .clone()
                .filter(|_| p.agent.as_deref() == Some("claude"))
            else {
                continue;
            };
            if Some(&tab) == sv.home_tab.as_ref()
                || workspace
                    .as_ref()
                    .is_some_and(|w| p.workspace_id.as_ref() != Some(w))
            {
                continue;
            }
            sv.targets.entry(tab).or_insert_with(|| p.pane_id.clone());
        }
        let tabs: BTreeSet<String> = sv
            .targets
            .keys()
            .cloned()
            .chain(self.created.values().cloned())
            .collect();
        let me = home.map(|h| h.pane_id.as_str());
        for tab in tabs {
            let candidates: Vec<String> = snap
                .panes
                .iter()
                .filter(|p| {
                    p.tab_id.as_ref() == Some(&tab)
                        && p.agent.as_deref().is_none_or(str::is_empty)
                        && Some(p.pane_id.as_str()) != me
                })
                .map(|p| p.pane_id.clone())
                .collect();
            for p in candidates {
                match self.is_panel(&p).await {
                    Some(true) => sv.panels.entry(tab.clone()).or_default().push(p),
                    None => _ = sv.unknown.insert(tab.clone()),
                    Some(false) => {}
                }
            }
        }
        if single {
            let crowded: Vec<String> = sv
                .targets
                .iter()
                .filter(|(tab, agent)| {
                    let panels = sv.panels.get(*tab);
                    snap.panes.iter().any(|p| {
                        p.tab_id.as_ref() == Some(*tab)
                            && &p.pane_id != *agent
                            && !panels.is_some_and(|ps| ps.contains(&p.pane_id))
                    })
                })
                .map(|(t, _)| t.clone())
                .collect();
            for t in crowded {
                sv.targets.remove(&t);
            }
        }
        sv
    }
}
