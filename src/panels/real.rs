//! The manager's [`Ops`] against herdr itself, over the kit's client.

use super::fit;
use super::ops::{Ops, Shell};
use super::shell;
use super::snap::Snap;
use herdr_agent_kit::herdr_api::{Direction, Herdr, OpenPluginPane, PaneId, Placement, TabId};
use serde_json::json;
use std::path::PathBuf;

pub struct RealOps {
    pub herdr: Herdr,
    /// Our plugin id (`HERDR_PLUGIN_ID`, else `leavemyyard.herdr-panorama`).
    pub plugin_id: String,
    /// The plugin's root (`HERDR_PLUGIN_ROOT`): the panel's working directory.
    pub root: Option<PathBuf>,
    /// The panel's drawable width (`cockpit_width`, clamped).
    pub width: i64,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl Ops for RealOps {
    async fn snapshot(&self) -> Option<Snap> {
        self.herdr.snapshot().await.ok().map(Snap::from)
    }

    async fn runs_panel(&self, pane: &str) -> Option<bool> {
        let info = self.herdr.process_info(&PaneId::new(pane)).await.ok()?;
        shell::runs_panorama(&info, "panel")
    }

    async fn open_panel(&self, agent: &str) -> Result<String, String> {
        let mut open = OpenPluginPane::new(self.plugin_id.clone(), "panel");
        open.placement = Some(Placement::Split);
        open.direction = Some(Direction::Right);
        open.target_pane = Some(PaneId::new(agent));
        open.cwd = self.root.clone();
        open.focus = false;
        match self.herdr.open_plugin_pane(&open).await.map_err(err)? {
            Some(p) => p.pane_id.map(|p| p.as_str().to_owned()).ok_or_else(|| "no pane in the reply".into()),
            None => Err("no pane in the reply".into()),
        }
    }

    async fn swap(&self, source: &str, target: &str) -> Result<(), String> {
        self.herdr.swap_panes(&PaneId::new(source), &PaneId::new(target)).await.map(drop).map_err(err)
    }

    async fn close_plugin_pane(&self, pane: &str) -> Result<(), String> {
        self.herdr.close_plugin_pane(&PaneId::new(pane)).await.map_err(err)
    }

    async fn close_pane(&self, pane: &str) -> Result<(), String> {
        // herdr no longer counts a restored panel as a plugin pane: `pane.close`
        self.herdr.raw("pane.close", json!({"pane_id": pane})).await.map(drop).map_err(err)
    }

    async fn focus_tab(&self, tab: &str) -> Result<(), String> {
        self.herdr.focus_tab(&TabId::new(tab)).await.map(drop).map_err(err)
    }

    async fn focus_pane(&self, pane: &str) -> Result<(), String> {
        self.herdr.focus_pane(&PaneId::new(pane)).await.map(drop).map_err(err)
    }

    async fn fit(&self, panel: &str, tab: &str) {
        let Ok(lay) = self.herdr.layout_export(&TabId::new(tab)).await else { return };
        let Some(root) = lay.root else { return };
        let Ok(rects) = self.herdr.pane_layout(&PaneId::new(panel)).await else { return };
        let rects = rects.extra.get("panes").cloned().unwrap_or_default();
        if let Some((path, ratio)) = fit::ratio(&root, &rects, panel, self.width) {
            let _ = self.herdr.set_split_ratio(&TabId::new(tab), &path, ratio).await;
        }
    }

    async fn pane_shell(&self, pane: &str) -> Shell {
        match self.herdr.process_info(&PaneId::new(pane)).await {
            Ok(info) => shell::plain_shell(&info),
            Err(_) => Shell::Unknown,
        }
    }

    async fn shell_state(&self, pid: i64) -> (Option<f64>, Option<bool>) {
        tokio::task::spawn_blocking(move || shell::shell_state(pid)).await.unwrap_or((None, None))
    }
}
