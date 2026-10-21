//! What the manager asks of herdr and of the system, as one trait: herdr itself
//! ([`super::real::RealOps`]) or an in-memory herdr in the tests (Python's `World`).

use super::snap::Snap;
use std::future::Future;

/// A pane's foreground as the restore check reads it (Python `herdr.pane_shell`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
    /// herdr cannot say yet (mid-exec, mid-restore).
    Unknown,
    /// Something other than the pane's own plain shell runs in it.
    Busy,
    /// Only the pane's own plain shell, with its pid.
    Plain(i64),
}

pub trait Ops: Send {
    /// herdr's snapshot, or `None` when herdr did not answer.
    fn snapshot(&self) -> impl Future<Output = Option<Snap>> + Send;
    /// Whether the pane's foreground runs `panorama panel` (`None`: herdr cannot say).
    fn runs_panel(&self, pane: &str) -> impl Future<Output = Option<bool>> + Send;
    /// A panel split right of `agent` without focus: the new pane's id, or why not.
    fn open_panel(&self, agent: &str) -> impl Future<Output = Result<String, String>> + Send;
    /// Swaps two panes; herdr focuses the source and shows its tab.
    fn swap(&self, source: &str, target: &str) -> impl Future<Output = Result<(), String>> + Send;
    /// Closes one of our plugin panes.
    fn close_plugin_pane(&self, pane: &str) -> impl Future<Output = Result<(), String>> + Send;
    /// Closes any pane (a restored panel is no plugin pane any more).
    fn close_pane(&self, pane: &str) -> impl Future<Output = Result<(), String>> + Send;
    fn focus_tab(&self, tab: &str) -> impl Future<Output = Result<(), String>> + Send;
    fn focus_pane(&self, pane: &str) -> impl Future<Output = Result<(), String>> + Send;
    /// The panel's split at its width (Python `cockpit.fit`); best effort.
    fn fit(&self, panel: &str, tab: &str) -> impl Future<Output = ()> + Send;
    /// The pane's foreground for the restore check (Python `herdr.pane_shell`).
    fn pane_shell(&self, pane: &str) -> impl Future<Output = Shell> + Send;
    /// A shell's age in seconds and whether it has children (Python `herdr.shell_state`);
    /// `None` where it cannot be read.
    fn shell_state(&self, shell: i64) -> impl Future<Output = (Option<f64>, Option<bool>)> + Send;
}
