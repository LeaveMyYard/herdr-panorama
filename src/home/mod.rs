//! Panorama's home surface (Python `home.py`, and `cockpit.py`'s per-tab settings and
//! `undock`; docs/DESIGN.md "Home tab"). SAFETY: it opens, renames and moves panorama's own
//! pane and renames tabs.
//!
//! - **One home, found by content.** The home pane carries the pane label `◆ panorama`,
//!   runs no agent, and herdr's process info shows `panorama tui` in its foreground. Every
//!   such pane is a candidate, wherever it sits. Look-alikes (a shell with the label, another
//!   plugin's pane) are never touched. One is used (the caller's own, else the recorded one,
//!   else the first); a second verified panorama is left alone. An older home pane (the
//!   only agent-less pane of a tab labelled `◆ panorama`) is adopted and labelled.
//! - **Opening** runs under the home lock and re-reads herdr after taking it, so the
//!   startup hook and the key never open two: the plugin's `panorama` pane in a tab of its
//!   own (`PANORAMA_HOME=1`), renamed `◆ panorama`, moved first.
//! - **Never a stale label:** a `◆ panorama` tab without panorama in it is renamed (the
//!   sweep in every `ensure`, and on quit).
//! - **Back home:** a home an older version's moving cockpit left beside an agent goes back
//!   to a tab of its own (Q187: per-tab panels are the only cockpit).
//! - The record (`home-<session>.json`) is only a hint.

pub mod config;
mod ensure;
mod find;
pub mod label;
pub mod ops;
mod open;
pub mod real;
mod record;

pub use ensure::{ensure, find_home, leave};
pub use find::{lookalikes, release_tab, sweep};
pub use open::{Opened, open_key, startup};
pub use ops::{At, HomeOps};

use herdr_agent_kit::internal::plugin_paths::panorama as paths;
use herdr_agent_kit::plugin_env::PluginEnv;
use std::path::PathBuf;
use std::time::Duration;

/// The home tab's label, and the home pane's own label.
pub const HOME_LABEL: &str = "◆ panorama";
/// The longest any caller waits for the home lock.
pub const LOCK: Duration = Duration::from_secs(10);
/// Quit (on the UI's path) waits at most this long.
pub const QUIT_LOCK: Duration = Duration::from_millis(500);

/// Panorama's home pane and its tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Home {
    pub pane_id: String,
    pub tab_id: String,
}

/// The per-session record and lock (Python `home._record`, `home._lock`).
#[derive(Clone, Debug)]
pub struct Places {
    /// `home-<session>.json` in the state dir.
    pub record: PathBuf,
    /// `home-<session>.lock` in the state dir.
    pub lock: PathBuf,
    /// How long `ensure` waits for the lock (`LOCK`).
    pub wait: Duration,
}

impl Places {
    /// The record and lock of this herdr session (one per `HERDR_SOCKET_PATH`).
    pub fn from_env(env: &PluginEnv) -> Self {
        let key = paths::session_key(env);
        Self {
            record: paths::state_dir(env).join(format!("home-{key}.json")),
            lock: paths::home_lock_path(env),
            wait: LOCK,
        }
    }
}

#[cfg(test)]
mod tests;
