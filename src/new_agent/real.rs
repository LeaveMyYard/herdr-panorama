//! Starting an agent against herdr itself, over the kit's client.

use super::ops::{NewTab, StartOps};
use crate::panels::Ops;
use crate::panels::real::RealOps;
use crate::trace::HerdrClient;
use herdr_agent_kit::herdr_api::{PaneId, Sanitized, TabId};
use herdr_agent_kit::plugin_env::PluginEnv;
use serde_json::{Map, Value, json};

/// The panel's command (the manifest's `panel` entrypoint, `scripts/panel`).
pub const PANEL_COMMAND: [&str; 3] = [
    "sh",
    "-c",
    "exec sh \"${HERDR_PLUGIN_ROOT:-.}/scripts/panel\"",
];
/// The share of the new tab the panel starts with (Python `new_agent`'s 0.3); `fit` sets its
/// width right after.
pub const PANEL_RATIO: f64 = 0.3;
/// How long the agent may take to become ready (Python `start_agent`'s 60 s).
pub const START_TIMEOUT_MS: u32 = 60_000;
/// A tab label's longest (Python `clean(label, 200)`).
pub const LABEL_MAX: usize = 200;

/// The plugin's own variables that are set (`HERDR_PLUGIN_ID`, `_ROOT`, `_STATE_DIR`,
/// `_CONFIG_DIR`; `var` reads an empty one as unset): a new panel's environment.
pub fn plugin_env(env: &PluginEnv) -> Vec<(String, String)> {
    [
        "HERDR_PLUGIN_ID",
        "HERDR_PLUGIN_ROOT",
        "HERDR_PLUGIN_STATE_DIR",
        "HERDR_PLUGIN_CONFIG_DIR",
    ]
    .into_iter()
    .filter_map(|k| env.var(k).map(|v| (k.to_owned(), v.to_owned())))
    .collect()
}

pub struct RealStart {
    pub herdr: HerdrClient,
    /// The plugin's own variables (`HERDR_PLUGIN_ID`, `_ROOT`, `_STATE_DIR`, `_CONFIG_DIR`)
    /// that are set: the panel's environment.
    pub plugin_env: Vec<(String, String)>,
    /// `HERDR_PLUGIN_ROOT`: the panel's directory.
    pub root: Option<String>,
    /// The panels' side (`fit`).
    pub panels: RealOps,
    /// How long the agent may take to become ready ([`START_TIMEOUT_MS`]).
    pub start_timeout_ms: u32,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl StartOps for RealStart {
    async fn agent_exists(&self, name: &str) -> bool {
        matches!(self.herdr.agent_named(name).await, Ok(Some(_)))
    }

    async fn panel_layout(&self, label: &str, cwd: Option<&str>) -> Result<NewTab, String> {
        let env: Map<String, Value> = self
            .plugin_env
            .iter()
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect();
        let mut first = json!({"type": "pane", "command": PANEL_COMMAND, "env": env, "label": crate::panels::PANEL_LABEL});
        if let Some(r) = &self.root {
            first["cwd"] = json!(r);
        }
        let mut second = json!({"type": "pane"});
        if let Some(c) = cwd {
            second["cwd"] = json!(c);
        }
        let root = json!({"type": "split", "direction": "right", "ratio": PANEL_RATIO, "first": first, "second": second});
        let label = Sanitized::with_limit(label, LABEL_MAX);
        let lay = self
            .herdr
            .apply_layout(&label, false, root)
            .await
            .map_err(err)?;
        let tab = lay.tab_id.ok_or("no tab in the reply")?.as_str().to_owned();
        let id = |side: &str| {
            lay.root
                .as_ref()
                .and_then(|r| r[side]["pane_id"].as_str())
                .map(str::to_owned)
        };
        Ok(NewTab {
            tab,
            pane: id("second"),
            panel: id("first"),
        })
    }

    async fn plain_tab(&self, label: &str, cwd: Option<&str>) -> Result<NewTab, String> {
        let label = Sanitized::new(label);
        let t = self
            .herdr
            .create_tab(&label, cwd.map(std::path::Path::new), false, None)
            .await
            .map_err(err)?;
        let tab = t.tab_id.ok_or("no tab in the reply")?.as_str().to_owned();
        let panes = self.herdr.panes().await.unwrap_or_default();
        let pane = panes
            .into_iter()
            .find(|p| p.tab_id.as_ref().is_some_and(|t| t.as_str() == tab))
            .and_then(|p| p.pane_id)
            .map(|p| p.as_str().to_owned());
        Ok(NewTab {
            tab,
            pane,
            panel: None,
        })
    }

    async fn agent_in(&self, pane: &str) -> Option<bool> {
        self.herdr
            .agent(&PaneId::new(pane))
            .await
            .ok()
            .map(|a| a.is_some())
    }

    async fn fit(&self, panel: &str, tab: &str) {
        self.panels.fit(panel, tab).await;
    }

    async fn start_agent(&self, name: &str, kind: &str, pane: &str) -> Result<(), String> {
        self.herdr
            .start_agent(
                &Sanitized::new(name),
                kind,
                &PaneId::new(pane),
                self.start_timeout_ms,
            )
            .await
            .map(drop)
            .map_err(err)
    }

    async fn close_tab(&self, tab: &str) -> Result<(), String> {
        self.herdr.close_tab(&TabId::new(tab)).await.map_err(err)
    }

    async fn focus_tab(&self, tab: &str) -> Result<(), String> {
        self.herdr
            .focus_tab(&TabId::new(tab))
            .await
            .map(drop)
            .map_err(err)
    }

    async fn focus_pane(&self, pane: &str) -> Result<(), String> {
        self.herdr
            .focus_pane(&PaneId::new(pane))
            .await
            .map(drop)
            .map_err(err)
    }
}
