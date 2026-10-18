//! The open key and the startup hook (Python `cli.cmd_open` without the core's part, and
//! `cli.cmd_home`): the home tab when it is on, else, or when it cannot be opened, the popup.

use super::config::home_enabled;
use super::ensure::ensure;
use super::ops::HomeOps;
use super::Places;
use serde_json::{Map, Value};

/// What the open key showed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Opened {
    /// The home tab, focused.
    Home,
    /// The popup: the home tab is off, or it could not be shown (why).
    Popup(Option<String>),
}

/// The open key (the core, when per-tab panels run, answers first; PR 8): the home tab,
/// opened if missing; the popup when the home tab is off or `ensure` failed. An error only
/// when the popup could not be opened either.
pub async fn open_key(ops: &impl HomeOps, places: &Places, cfg: &Map<String, Value>) -> Result<Opened, String> {
    let why = if home_enabled(cfg) {
        match ensure(ops, places, true).await {
            Ok(_) => return Ok(Opened::Home),
            Err(e) => Some(e),
        }
    } else {
        None
    };
    ops.open_popup().await.map_err(|e| format!("could not open the view: {e}"))?;
    Ok(Opened::Popup(why))
}

/// herdr's startup hook: the home tab once, without taking the focus (nothing when off).
pub async fn startup(ops: &impl HomeOps, places: &Places, cfg: &Map<String, Value>) -> Result<(), String> {
    if !home_enabled(cfg) {
        return Ok(());
    }
    ensure(ops, places, false).await.map(drop)
}
