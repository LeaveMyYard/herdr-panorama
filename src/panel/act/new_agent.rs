//! The + New agent form in its own herdr popup (Python `herdr.open_new_agent`,
//! `SwarmApp.action_new_agent`, `_popup_started`): the plugin's `new-agent` entrypoint, with
//! the directory it starts in. When herdr does not answer, it may have opened it all the
//! same: the popup's process marks that it started, and only if it never does is the
//! operator told to press n again.

use herdr_agent_kit::herdr_api::{Herdr, HerdrError, OpenPluginPane, Placement};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The popup's outer size in cells (the form is 76 wide).
pub const POPUP: (u16, u16) = (84, 26);
/// After an unanswered request: how long the popup's process may take to start.
pub const POPUP_START: Duration = Duration::from_secs(3);
/// The operator's wording (Q154).
pub const DID_NOT_OPEN: &str = "herdr did not open the New agent popup; press n again";

/// Where the popup's process marks that it started (Python `NewAgentApp._marker`).
pub fn marker(state_dir: &Path, session_key: &str) -> PathBuf {
    state_dir.join(format!("new-agent-{session_key}.started"))
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

/// Whether the popup's process started at or after `t0` (POSIX seconds).
pub fn started_since(marker: &Path, t0: f64) -> bool {
    std::fs::read_to_string(marker)
        .ok()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .is_some_and(|t| t >= t0)
}

/// Opens the popup for `plugin_id` (run from `root`), starting in `cwd`. The notice to show,
/// if any: herdr refused (its words), or it did not answer and no popup started.
pub async fn open(
    h: &Herdr,
    plugin_id: &str,
    root: Option<&Path>,
    cwd: &str,
    marker: &Path,
) -> Option<String> {
    let t0 = now();
    let mut open = OpenPluginPane::new(plugin_id, "new-agent");
    open.placement = Some(Placement::Popup);
    open.focus = true;
    open.size = Some(POPUP);
    open.env = vec![("PANORAMA_NEW_AGENT_CWD".into(), cwd.into())];
    open.cwd = root.map(Path::to_path_buf);
    match h.open_plugin_pane(&open).await {
        Ok(_) => None,
        Err(HerdrError::NoReply(_)) => {
            // it may be open: look for the popup's own mark before saying anything
            let deadline = tokio::time::Instant::now() + POPUP_START;
            while tokio::time::Instant::now() < deadline {
                if started_since(marker, t0) {
                    return None;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            (!started_since(marker, t0)).then(|| DID_NOT_OPEN.to_owned())
        }
        Err(e) => Some(e.to_string()),
    }
}

#[cfg(test)]
mod tests;
