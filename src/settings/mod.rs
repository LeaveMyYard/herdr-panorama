//! What the panel reads from the two `config.toml` files, re-read when they change:
//!
//! - panorama's own, in the plugin config dir ([`panel`]): the panel's colours and
//!   `animation_fps`;
//! - herdr's (`$XDG_CONFIG_HOME/herdr`, else `~/.config/herdr`): its theme ([`theme`]) and
//!   the key bound to panorama's open action ([`open_key`]).
//!
//! Both are parsed as TOML 1.0, as Python's `tomllib` does (the kit's `ConfigFile`); a
//! missing or broken file reads as empty, so everything takes its default.

pub mod colour;
pub mod open_key;
pub mod panel;
pub mod theme;

use herdr_agent_kit::internal::plugin_paths::PANORAMA_ID;
use herdr_agent_kit::plugin_env::{ConfigFile, PluginEnv};
use panorama_core::view::{PanelSettings, Tokens};
use serde_json::{Map, Value};
use std::path::PathBuf;

/// The two config files, cached until they change.
#[derive(Debug)]
pub struct Settings {
    panorama: ConfigFile,
    herdr: ConfigFile,
    plugin_id: String,
}

fn table(file: &mut ConfigFile) -> Map<String, Value> {
    file.load_as().unwrap_or_default()
}

impl Settings {
    /// panorama's config in `panorama_dir` (none: all defaults), herdr's in `herdr_dir`;
    /// `plugin_id` names the open action (`$HERDR_PLUGIN_ID`, else panorama's id).
    pub fn new(panorama_dir: Option<PathBuf>, herdr_dir: PathBuf, plugin_id: &str) -> Self {
        Self {
            panorama: panorama_dir.map(ConfigFile::in_dir).unwrap_or_default(),
            herdr: ConfigFile::in_dir(herdr_dir),
            plugin_id: plugin_id.to_owned(),
        }
    }

    /// From the plugin environment. Without `$HERDR_PLUGIN_CONFIG_DIR` panorama's own file is
    /// not read (herdr sets it for every plugin pane).
    pub fn from_env(env: &PluginEnv) -> Self {
        let own = env.plugin_config_dir().map(|p| p.to_path_buf());
        Self::new(
            own,
            env.xdg_config_home().join("herdr"),
            env.plugin_id(PANORAMA_ID),
        )
    }

    /// The panel's colour settings.
    pub fn panel(&mut self) -> PanelSettings {
        panel::panel_settings(&table(&mut self.panorama))
    }

    /// How often colours move, per second.
    pub fn animation_fps(&mut self) -> f64 {
        panel::animation_fps(&table(&mut self.panorama))
    }

    /// herdr's theme tokens; `None` for its "terminal" theme.
    pub fn theme(&mut self) -> Option<Tokens> {
        theme::tokens(&table(&mut self.herdr))
    }

    /// The open key, `(full, short)`.
    pub fn open_key(&mut self) -> (String, String) {
        open_key::current(&table(&mut self.herdr), &self.plugin_id)
    }

    /// Why a config file was ignored, if one was (for the log).
    pub fn errors(&self) -> impl Iterator<Item = &str> {
        [self.panorama.error(), self.herdr.error()]
            .into_iter()
            .flatten()
    }
}

#[cfg(test)]
mod tests;
