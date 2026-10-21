//! What the home module asks of herdr, as one trait: herdr itself ([`super::real::RealHome`])
//! or the tests' in-memory herdr (Python's stateful fake CLI).

use crate::panels::snap::Snap;
use std::future::Future;

/// Where a pane is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct At {
    pub pane_id: String,
    pub tab_id: String,
}

pub trait HomeOps: Send + Sync {
    /// herdr's snapshot, or `None` when herdr did not answer.
    fn snapshot(&self) -> impl Future<Output = Option<Snap>> + Send;
    /// Whether the pane's foreground runs `panorama tui` (`None`: herdr cannot say).
    fn runs_tui(&self, pane: &str) -> impl Future<Output = Option<bool>> + Send;
    /// The plugin's `panorama` pane in a tab of its own (`PANORAMA_HOME=1`), focused or not.
    fn open_home_pane(&self, focus: bool) -> impl Future<Output = Result<At, String>> + Send;
    /// The plugin's `panorama` pane at its own placement (the popup).
    fn open_popup(&self) -> impl Future<Output = Result<(), String>> + Send;
    fn rename_pane(
        &self,
        pane: &str,
        label: &str,
    ) -> impl Future<Output = Result<(), String>> + Send;
    fn rename_tab(&self, tab: &str, label: &str)
    -> impl Future<Output = Result<(), String>> + Send;
    /// The tab to the first position (best effort: an older herdr has no `tab.move`).
    fn move_tab_first(&self, tab: &str) -> impl Future<Output = Result<(), String>> + Send;
    /// The pane into a new tab with `label` in `workspace`, without focus: where it is now.
    fn move_to_new_tab(
        &self,
        pane: &str,
        workspace: &str,
        label: &str,
    ) -> impl Future<Output = Result<At, String>> + Send;
    fn focus_tab(&self, tab: &str) -> impl Future<Output = Result<(), String>> + Send;
}
