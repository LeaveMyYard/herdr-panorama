//! Starting the agent (Python `home.new_agent` without its first prompt: approved deviation
//! 2026-10-17, nothing is ever typed into the pane): the tab, unfocused, the agent started in
//! its pane, then the tab focused. A failed start closes that tab again, so nothing is left
//! behind; only the tab this start made is ever closed.

use super::ops::StartOps;
use super::{name_ok, slugify};
use std::path::Path;

/// The agent named after `name` (slugified) in a new tab, `cwd` its directory; per-tab mode
/// builds the tab with its panel on the left from the first frame. Its name, or why not.
pub async fn new_agent(
    ops: &impl StartOps,
    name: &str,
    cwd: Option<&str>,
    kind: &str,
    per_tab: bool,
) -> Result<String, String> {
    let slug = slugify(name);
    if !name_ok(&slug) {
        return Err("the name needs at least one letter".into());
    }
    if let Some(c) = cwd.filter(|c| !Path::new(c).is_dir()) {
        return Err(format!("no such directory: {c}"));
    }
    if ops.agent_exists(&slug).await {
        return Err(format!("an agent named {slug} already exists"));
    }
    let made = if per_tab {
        ops.panel_layout(&slug, cwd).await
    } else {
        ops.plain_tab(&slug, cwd).await
    };
    let tab = made.map_err(|e| format!("tab create failed: {e}"))?;
    let Some(pane) = tab.pane.clone() else {
        let _ = ops.close_tab(&tab.tab).await;
        return Err("the new tab has no pane".into());
    };
    if let Some(panel) = &tab.panel {
        ops.fit(panel, &tab.tab).await;
    }
    if let Err(e) = ops.start_agent(&slug, kind, &pane).await {
        let _ = ops.close_tab(&tab.tab).await; // no orphan tab left behind
        return Err(format!("agent start failed: {e}"));
    }
    let _ = ops.focus_tab(&tab.tab).await;
    if tab.panel.is_some() {
        let _ = ops.focus_pane(&pane).await; // the agent, not the panel layout.apply left focused
    }
    Ok(slug)
}
