//! Before going to an orchestrator: does herdr still have its pane? (The core's state may be
//! old; Python `PanelApp.action_goto`.)

use crate::trace::HerdrClient;

/// Whether herdr's snapshot has `pane`. A snapshot that cannot be read counts as not (Python
/// reads it as empty).
pub async fn exists(h: &HerdrClient, pane: &str) -> bool {
    h.snapshot().await.is_ok_and(|snap| {
        snap.panes
            .iter()
            .any(|p| p.pane_id.as_ref().is_some_and(|id| id.as_str() == pane))
    })
}

/// The notice when it is gone.
pub fn gone(label: &str) -> String {
    format!("{label} is gone")
}

#[cfg(test)]
mod tests;
