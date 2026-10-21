//! One pass (Python `Manager._reconcile`): forget what is gone, notice panels the operator
//! closed, close duplicates and orphans, and place what is missing (the start batch, else
//! only the operator's tab).

use super::Manager;
use super::ops::Ops;
use super::snap::Snap;
use super::survey::{HomeRef, Survey};
use panorama_core::model::Swarm;
use std::collections::BTreeSet;

impl<O: Ops> Manager<O> {
    /// One pass over `snap`. `start`: the core just started (the start batch). `verified`: the
    /// panes whose viewers said a verified hello (the core's). `single`: see `survey`.
    pub(super) async fn reconcile_unlocked(
        &mut self,
        snap: Snap,
        swarm: &Swarm,
        home: Option<&HomeRef>,
        verified: &BTreeSet<String>,
        start: bool,
        single: bool,
    ) -> Survey {
        let now = (self.clock)();
        self.passes += 1;
        let snap = self.restored(snap).await;
        let mut sv = self.survey(&snap, swarm, home, single).await;
        let present: BTreeSet<&str> = snap.panes.iter().map(|p| p.pane_id.as_str()).collect();
        // adopted: a verified viewer's panel (after a core restart)
        for (tab, ps) in &sv.panels {
            for p in ps.iter().filter(|p| verified.contains(*p)) {
                self.created.entry(p.clone()).or_insert_with(|| tab.clone());
            }
        }
        let ours_live: Vec<String> = verified
            .iter()
            .filter(|p| self.created.contains_key(*p))
            .cloned()
            .collect();
        self.connected_once.extend(ours_live);
        let gone: Vec<(String, String)> = self
            .created
            .iter()
            .filter(|(p, _)| !present.contains(p.as_str()))
            .map(|(p, t)| (p.clone(), t.clone()))
            .collect();
        for (panel, tab) in gone {
            self.created.remove(&panel);
            self.creating.remove(&tab); // that create is over
            if !sv.targets.contains_key(&tab) {
                continue;
            }
            // gone while its orchestrator is still there: the operator closed it, unless its
            // viewer died on its own (never connected, e.g. a broken environment, or left a
            // crash marker)
            if !self.connected_once.contains(&panel) || self.crashed(&panel) {
                let n = self.crashes.entry(tab.clone()).or_default();
                *n += 1;
                let n = *n;
                let more = if n >= super::CRASH_LIMIT {
                    "; no more panels there"
                } else {
                    ""
                };
                self.note(format!(
                    "the panel in {tab} died on its own ({n} of {}){more}",
                    super::CRASH_LIMIT
                ));
                if n >= super::CRASH_LIMIT {
                    self.broken.insert(tab);
                }
                continue;
            }
            self.saved.closed.insert(tab);
            self.save();
        }
        let before = self.saved.closed.len();
        self.saved.closed.retain(|t| sv.tabs.contains(t)); // tabs that no longer exist
        if self.saved.closed.len() != before {
            self.save();
        }
        self.creating
            .retain(|t, until| *until > now && !sv.panels.contains_key(t));
        self.right.retain(|t| sv.panels.contains_key(t));
        self.close_extras(&mut sv, verified, now).await;
        self.place_missing(&mut sv, start).await;
        self.remember(&snap, &sv);
        sv
    }

    /// Duplicates (keep one, a connected one first; close the provable rest) and orphans (a
    /// tab whose orchestrator went away keeps its panel for `REMOVE_AFTER`).
    async fn close_extras(
        &mut self,
        sv: &mut Survey,
        verified: &BTreeSet<String>,
        now: std::time::Instant,
    ) {
        let tabs: Vec<String> = sv.panels.keys().cloned().collect();
        for tab in &tabs {
            let ps = sv.panels[tab].clone();
            if ps.len() > 1 {
                let keep = ps
                    .iter()
                    .find(|p| verified.contains(*p))
                    .unwrap_or(&ps[0])
                    .clone();
                for p in ps.iter().filter(|p| **p != keep) {
                    if self.ours(p, verified) && self.ops.close_plugin_pane(p).await.is_ok() {
                        self.created.remove(p);
                        self.note(format!("closed a second panel in {tab}"));
                    }
                }
                sv.panels.insert(tab.clone(), vec![keep]);
            }
        }
        for tab in &tabs {
            if sv.targets.contains_key(tab) {
                self.orphaned.remove(tab);
                continue;
            }
            let since = *self.orphaned.entry(tab.clone()).or_insert(now);
            if now.duration_since(since) >= super::REMOVE_AFTER {
                for p in sv.panels[tab].clone() {
                    if self.ours(&p, verified) && self.ops.close_plugin_pane(&p).await.is_ok() {
                        self.created.remove(&p);
                    }
                }
                self.orphaned.remove(tab);
                sv.panels.remove(tab);
            }
        }
        self.orphaned.retain(|t, _| sv.panels.contains_key(t));
    }

    /// The missing panels: all of them in the start batch, else only in the operator's tab
    /// (or a panel left on the right there is swapped left now).
    async fn place_missing(&mut self, sv: &mut Survey, start: bool) {
        let missing: Vec<(String, String)> = sv
            .targets
            .iter()
            .filter(|(t, _)| {
                !sv.panels.contains_key(*t)
                    && !sv.unknown.contains(*t)
                    && !self.saved.closed.contains(*t)
                    && !self.creating.contains_key(*t)
                    && !self.broken.contains(*t)
            })
            .map(|(t, a)| (t.clone(), a.clone()))
            .collect();
        if start {
            self.started = true;
            for panel in self.batch(&missing, sv).await {
                let tab = self.created[&panel].clone();
                sv.panels.entry(tab).or_default().push(panel);
            }
        } else if let Some(tab) = sv.focused_tab.clone() {
            if let Some((_, agent)) = missing.iter().find(|(t, _)| *t == tab) {
                if let Some(panel) = self.place(&tab, &agent.clone(), true).await {
                    sv.panels.entry(tab).or_default().push(panel);
                }
            } else if self.right.contains(&tab)
                // as Python: the tab's first panel, which is a look-alike if ours went (#101)
                && let (Some(agent), Some(panel)) = (
                    sv.targets.get(&tab).cloned(),
                    sv.panels.get(&tab).and_then(|p| p.first()).cloned(),
                )
            {
                self.swap_left(&tab, &agent, &panel).await;
            }
        }
    }
}
