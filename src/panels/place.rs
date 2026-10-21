//! Placing panels (Python `Manager.place`, `_swap_left`, `batch`): the one placement is a
//! split right of the agent without focus, a swap with the agent as the swap's source (herdr
//! focuses a swap's source and shows its tab), then the width.

use super::Manager;
use super::focus_log::Focus;
use super::ops::Ops;
use super::survey::Survey;
use std::time::Instant;

/// The first focus event that is not the batch's own: the batch's swaps cause `expected` in
/// order (herdr may leave one out, e.g. no tab event when that tab is already shown); an event
/// that is not the next of them, or a later one, is the operator's.
pub fn operator_moved(events: &[Focus], expected: &[Focus]) -> Option<String> {
    let mut i = 0;
    for ev in events {
        let id = match ev {
            Focus::Tab(t) | Focus::Pane(t) if !t.is_empty() => t,
            _ => continue,
        };
        match (i..expected.len()).find(|&k| &expected[k] == ev) {
            Some(j) => i = j + 1,
            None => return Some(id.clone()),
        }
    }
    None
}

impl<O: Ops> Manager<O> {
    /// The one placement. `lazy` (the operator is in this tab): look again right before the
    /// swap; if they have left, leave the panel on the right for now; if they were on another
    /// pane of the tab, give it the focus back. The panel's pane id.
    pub async fn place(&mut self, tab: &str, agent: &str, lazy: bool) -> Option<String> {
        self.creating
            .insert(tab.to_owned(), (self.clock)() + super::CREATING);
        let panel = match self.ops.open_panel(agent).await {
            Ok(p) => p,
            Err(e) => {
                self.note(format!("no panel for {tab}: {e}"));
                return None;
            }
        };
        self.created.insert(panel.clone(), tab.to_owned());
        let mut was = None;
        if lazy {
            let now = self.ops.snapshot().await.unwrap_or_default();
            if now.focused_tab.as_deref() != Some(tab) {
                self.right.insert(tab.to_owned());
                self.ops.fit(&panel, tab).await;
                return Some(panel);
            }
            was = now.focused_pane;
        }
        self.swap_left(tab, agent, &panel).await;
        if let Some(w) = was.filter(|w| w != agent && *w != panel) {
            let _ = self.ops.focus_pane(&w).await;
        }
        Some(panel)
    }

    pub(super) async fn swap_left(&mut self, tab: &str, agent: &str, panel: &str) {
        // the agent is the source: it keeps the focus
        if let Err(e) = self.ops.swap(agent, panel).await {
            self.note(format!("the panel in {tab} stays right of its agent: {e}"));
            self.right.insert(tab.to_owned());
        } else {
            self.right.remove(tab);
        }
        self.ops.fit(panel, tab).await;
    }

    /// The start batch: every missing panel, then the operator back where they were, unless a
    /// focus event that is not one of the batch's own shows they went elsewhere.
    pub(super) async fn batch(&mut self, missing: &[(String, String)], sv: &Survey) -> Vec<String> {
        let t0 = Instant::now();
        let (was_tab, was_pane) = (sv.focused_tab.clone(), sv.focused_pane.clone());
        let mut expected = vec![];
        let mut made = vec![];
        for (tab, agent) in missing {
            if let Some(panel) = self.place(tab, agent, false).await {
                made.push(panel);
                expected.extend([Focus::Tab(tab.clone()), Focus::Pane(agent.clone())]);
            }
        }
        let Some(was_tab) = was_tab.filter(|_| !made.is_empty()) else {
            return made;
        };
        if let Some(log) = self.focus_log.clone().filter(|l| l.live()) {
            let last = expected.last().cloned();
            log.wait_for(t0, |e| Some(e) == last.as_ref(), super::SYNC)
                .await;
            if let Some(went) = operator_moved(&log.since(t0), &expected) {
                self.note(format!(
                    "panel batch: the operator went to {went}; left there"
                ));
                return made;
            }
        }
        let mut err = self.ops.focus_tab(&was_tab).await.err();
        if err.is_none()
            && let Some(p) = was_pane
        {
            err = self.ops.focus_pane(&p).await.err();
        }
        if let Some(e) = err {
            self.note(format!("panel batch: could not put the focus back: {e}"));
        }
        made
    }
}
