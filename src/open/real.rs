//! [`OpenOps`] against herdr and the core.

use super::{OPEN_WAIT, OpenOps, request_open};
use crate::home::Places;
use crate::home::real::RealHome;
use herdr_agent_kit::herdr_api::{Direction, Herdr, OpenPluginPane, PaneId, Placement};
use herdr_agent_kit::internal::plugin_paths::PANORAMA_ID;
use herdr_agent_kit::plugin_env::PluginEnv;
use serde_json::Value;
use std::path::PathBuf;

pub struct RealOpen {
    pub home: RealHome,
    pub places: Places,
    /// The core's socket and our uid.
    pub core: (PathBuf, u32),
    /// The pane the action was invoked from (herdr's `focused_pane_id`, else `HERDR_PANE_ID`):
    /// the one `--split` splits.
    pub invoked_from: Option<String>,
}

/// herdr's `focused_pane_id` in the invocation context (`HERDR_PLUGIN_CONTEXT_JSON`).
pub fn invocation_pane(context_json: Option<&str>) -> Option<String> {
    let ctx: Value = serde_json::from_str(context_json?).ok()?;
    ctx.get("focused_pane_id")?
        .as_str()
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
}

impl RealOpen {
    pub fn from_env(env: &PluginEnv) -> Self {
        Self {
            home: RealHome {
                herdr: Herdr::from_env().with_client("panorama"),
                plugin_id: env.plugin_id(PANORAMA_ID).to_owned(),
                root: env.plugin_root().map(PathBuf::from),
            },
            places: Places::from_env(env),
            core: (crate::proto::socket::path(env), env.uid()),
            invoked_from: invocation_pane(env.plugin_context_json())
                .or_else(|| env.var("HERDR_PANE_ID").map(str::to_owned)),
        }
    }

    /// The `panorama` pane to open, focused (`OpenPluginPane::new`'s default): its own
    /// placement, or a right split of the pane the action came from.
    pub fn view(&self, split: bool) -> OpenPluginPane {
        let mut open = OpenPluginPane::new(self.home.plugin_id.clone(), "panorama");
        open.cwd = self.home.root.clone();
        if split {
            open.placement = Some(Placement::Split);
            open.direction = Some(Direction::Right);
            open.target_pane = self.invoked_from.as_deref().map(PaneId::new);
        }
        open
    }
}

impl OpenOps for RealOpen {
    async fn request_open(&self) -> Option<String> {
        request_open(&self.core.0, self.core.1, OPEN_WAIT).await
    }

    async fn ensure_home(&self, focus: bool) -> Result<(), String> {
        super::ensure_home(&self.home, &self.places, focus).await
    }

    async fn open_view(&self, split: bool) -> Result<(), String> {
        self.home
            .herdr
            .open_plugin_pane(&self.view(split))
            .await
            .map(drop)
            .map_err(|e| e.to_string())
    }
}
