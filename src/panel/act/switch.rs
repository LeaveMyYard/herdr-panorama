//! A switch from a panel (Python `PanelApp._switch_worker`, `_prepaint`): instead of moving a
//! pane, show the target's tab with the keyboard on that tab's panel (`keep_focus`) or on
//! its agent, after a fresh look at herdr.
//!
//! Onto a panel, the switch is announced first (`focusing`, protocol 1.2): the core publishes
//! it, the target panel paints its focused look while still hidden, and herdr shows it with
//! that look on its first frame. The switcher waits for the core's echo (at most
//! [`PREPAINT_WAIT`]), then [`PREPAINT_PAINT`] for the paint. A switch that then fails takes
//! the announcement back, so the core stops naming the target.

use crate::trace::HerdrClient;
use herdr_agent_kit::herdr_api::{PaneId, TabId};
use std::collections::HashMap;
use std::time::Duration;

/// A switch waits at most this long for the core to echo its announcement.
pub const PREPAINT_WAIT: Duration = Duration::from_millis(150);
/// Then this long for the target panel to paint (a frame of herdr is ~20 ms).
pub const PREPAINT_PAINT: Duration = Duration::from_millis(20);

/// Announcing a switch's focus to the core.
pub trait Announce {
    /// Announces `pane` (`focusing`); whether it went out.
    fn focusing(&self, pane: &str) -> impl Future<Output = bool>;
    /// Whether the core echoes `pane` as the focused pane within `within`. An echo received
    /// since the matching [`Announce::focusing`] was sent counts (Python clears its echo and
    /// names the awaited pane before sending): an implementation that only waits for states
    /// arriving after this call would miss a fast echo and wait the whole `within`.
    fn echoed(&self, pane: &str, within: Duration) -> impl Future<Output = bool>;
}

/// How a switch ended.
#[derive(Debug, PartialEq, Eq)]
pub struct Switched {
    /// Why it failed.
    pub err: Option<String>,
    /// The announcement was taken back: the switching panel is the focused one again.
    pub took_back: bool,
}

/// Switches to the tab of the orchestrator in `target`. `panes_of`: tab -> its panel;
/// `my_pane`: the switching panel's own pane.
pub async fn switch(
    h: &HerdrClient,
    target: &str,
    keep_focus: bool,
    panes_of: &HashMap<String, String>,
    my_pane: Option<&str>,
    announce: &impl Announce,
) -> Switched {
    let failed = |err: String| Switched {
        err: Some(err),
        took_back: false,
    };
    let panes = h.snapshot().await.map(|s| s.panes).unwrap_or_default(); // unreadable: as empty
    let id = |p: &herdr_agent_kit::herdr_api::PaneInfo| {
        p.pane_id.as_ref().map(|i| i.as_str().to_owned())
    };
    let Some(tab) = panes
        .iter()
        .find(|p| id(p).as_deref() == Some(target))
        .and_then(|p| p.tab_id.clone())
        .filter(|t| !t.as_str().is_empty())
    // Python's `not agent.get("tab_id")`
    else {
        return failed(format!("{target} is gone"));
    };
    let panel = panes_of.get(tab.as_str());
    let here = |pane: &str| {
        panes
            .iter()
            .any(|p| p.tab_id.as_ref() == Some(&tab) && id(p).as_deref() == Some(pane))
    };
    match panel.filter(|p| keep_focus && here(p)) {
        Some(panel) => {
            let announced = prepaint(panel, announce).await;
            let err = focus_panel(h, &tab, panel).await;
            let took_back = err.is_some() && announced && my_pane.is_some();
            if let (true, Some(mine)) = (took_back, my_pane) {
                // nothing moved: this panel, still the focused one, keeps its look
                announce.focusing(mine).await;
            }
            Switched { err, took_back }
        }
        // into the agent; a tab without a panel yet gets one there (the core)
        None => Switched {
            err: h
                .focus_agent(&PaneId::new(target))
                .await
                .err()
                .map(|e| e.to_string()),
            took_back: false,
        },
    }
}

/// Announces the target panel and waits for the core's echo, then for its paint. Whether
/// the announcement went out.
async fn prepaint(panel: &str, announce: &impl Announce) -> bool {
    let sent = announce.focusing(panel).await;
    if sent && announce.echoed(panel, PREPAINT_WAIT).await {
        tokio::time::sleep(PREPAINT_PAINT).await;
    }
    sent
}

/// Shows `tab` with the keyboard on `panel`.
async fn focus_panel(h: &HerdrClient, tab: &TabId, panel: &str) -> Option<String> {
    if let Err(e) = h.focus_tab(tab).await {
        return Some(e.to_string());
    }
    h.focus_pane(&PaneId::new(panel))
        .await
        .err()
        .map(|e| e.to_string())
}

#[cfg(test)]
mod tests;
