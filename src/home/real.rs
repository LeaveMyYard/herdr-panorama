//! The home module's [`HomeOps`] against herdr itself, over the kit's client.

use super::ops::{At, HomeOps};
use crate::panels::shell::runs_panorama;
use crate::panels::snap::Snap;
use herdr_agent_kit::herdr_api::{
    Herdr, MoveTo, OpenPluginPane, PaneId, PaneInfo, Placement, Sanitized, TabId, WorkspaceId,
};
use std::path::PathBuf;

pub struct RealHome {
    pub herdr: Herdr,
    /// Our plugin id (`HERDR_PLUGIN_ID`, else `leavemyyard.herdr-panorama`).
    pub plugin_id: String,
    /// The plugin's root (`HERDR_PLUGIN_ROOT`): the pane's working directory.
    pub root: Option<PathBuf>,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn at(p: Option<PaneInfo>) -> Result<At, String> {
    let p = p.ok_or("no pane in the reply")?;
    Ok(At {
        pane_id: p.pane_id.ok_or("no pane in the reply")?.as_str().to_owned(),
        tab_id: p.tab_id.map(|t| t.as_str().to_owned()).unwrap_or_default(),
    })
}

impl RealHome {
    fn pane(&self) -> OpenPluginPane {
        let mut open = OpenPluginPane::new(self.plugin_id.clone(), "panorama");
        open.cwd = self.root.clone();
        open
    }
}

impl HomeOps for RealHome {
    async fn snapshot(&self) -> Option<Snap> {
        self.herdr.snapshot().await.ok().map(Snap::from)
    }

    async fn runs_tui(&self, pane: &str) -> Option<bool> {
        let info = self.herdr.process_info(&PaneId::new(pane)).await.ok()?;
        runs_panorama(&info, "tui")
    }

    async fn open_home_pane(&self, focus: bool) -> Result<At, String> {
        let mut open = self.pane();
        open.placement = Some(Placement::Tab);
        open.env = vec![("PANORAMA_HOME".into(), "1".into())];
        open.focus = focus;
        at(self.herdr.open_plugin_pane(&open).await.map_err(err)?)
    }

    async fn rename_pane(&self, pane: &str, label: &str) -> Result<(), String> {
        self.herdr
            .rename_pane(&PaneId::new(pane), &Sanitized::new(label))
            .await
            .map(drop)
            .map_err(err)
    }

    async fn rename_tab(&self, tab: &str, label: &str) -> Result<(), String> {
        self.herdr
            .rename_tab(&TabId::new(tab), &Sanitized::new(label))
            .await
            .map(drop)
            .map_err(err)
    }

    async fn move_tab_first(&self, tab: &str) -> Result<(), String> {
        self.herdr
            .move_tab(&TabId::new(tab), 0)
            .await
            .map(drop)
            .map_err(err)
    }

    async fn move_to_new_tab(
        &self,
        pane: &str,
        workspace: &str,
        label: &str,
    ) -> Result<At, String> {
        let to = MoveTo::NewTab {
            workspace: Some(WorkspaceId::new(workspace)),
            label: Sanitized::new(label),
        };
        let moved = self
            .herdr
            .move_pane(&PaneId::new(pane), &to, false)
            .await
            .map_err(err)?;
        at(moved.pane)
    }

    async fn focus_tab(&self, tab: &str) -> Result<(), String> {
        self.herdr
            .focus_tab(&TabId::new(tab))
            .await
            .map(drop)
            .map_err(err)
    }
}
