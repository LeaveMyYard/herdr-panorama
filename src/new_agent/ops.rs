//! What starting an agent asks of herdr (Python `herdr.agent_exists`, `apply_panel_layout`,
//! `create_tab`, `tab_root_pane`, `start_agent`, `close_tab`, `focus_tab`, `focus_pane`,
//! `cockpit.fit`), as one trait: herdr itself ([`super::real::RealStart`]) or the tests'.

use std::future::Future;

/// A new tab's panes: the agent's, and the panel's on its left in per-tab mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewTab {
    pub tab: String,
    pub pane: Option<String>,
    pub panel: Option<String>,
}

pub trait StartOps: Send + Sync {
    /// Whether herdr already has an agent by this name (`None`: it could not say).
    fn agent_exists(&self, name: &str) -> impl Future<Output = bool> + Send;
    /// A new unfocused tab `label` with a per-tab panel on the left and a plain pane on the
    /// right (`layout.apply`), the right one in `cwd`.
    fn panel_layout(
        &self,
        label: &str,
        cwd: Option<&str>,
    ) -> impl Future<Output = Result<NewTab, String>> + Send;
    /// A new unfocused tab `label` in `cwd`, and its pane.
    fn plain_tab(
        &self,
        label: &str,
        cwd: Option<&str>,
    ) -> impl Future<Output = Result<NewTab, String>> + Send;
    /// The panel's split at its width; best effort.
    fn fit(&self, panel: &str, tab: &str) -> impl Future<Output = ()> + Send;
    /// Whether herdr reports an agent in `pane`.
    fn agent_in(&self, pane: &str) -> impl Future<Output = bool> + Send;
    /// Claude Code named `name` in `pane`, waited for until it is ready.
    fn start_agent(
        &self,
        name: &str,
        kind: &str,
        pane: &str,
    ) -> impl Future<Output = Result<(), String>> + Send;
    fn close_tab(&self, tab: &str) -> impl Future<Output = Result<(), String>> + Send;
    fn focus_tab(&self, tab: &str) -> impl Future<Output = Result<(), String>> + Send;
    fn focus_pane(&self, pane: &str) -> impl Future<Output = Result<(), String>> + Send;
}
