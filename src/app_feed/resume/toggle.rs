//! The sidebar's toggle, read straight from the store it is kept in.
use crate::app_feed::fs::Home;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

/// Kept with the sidebar's other choices in the retained panorama-spaces store.
const TOGGLE_DIR: &str = "panorama-spaces";
pub(super) const TOGGLE_STORE: &str = "data/panorama-spaces/store.json";
const TOGGLE_KEY: &str = "agents.resume";
const CLICKS_KEY: &str = "agents.resume.clicks";
const TOGGLE_LIMIT: usize = 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Toggle {
    On,
    Off,
    /// Unreadable right now: neither act on it nor clear anything.
    Unknown,
}

pub(super) fn toggle(home: &Home) -> Toggle {
    // The mod makes its directory on first use; until then nothing can have been turned on.
    match home.names("data") {
        Ok(names) if names.iter().any(|n| n == TOGGLE_DIR) => {}
        Ok(_) => return Toggle::Off,
        Err(_) => return Toggle::Unknown,
    }
    let Ok(file) = home.read(TOGGLE_STORE, TOGGLE_LIMIT, 0o600) else {
        return Toggle::Unknown;
    };
    let Some(file) = file else {
        return Toggle::Off;
    };
    match serde_json::from_slice::<HashMap<String, Value>>(&file.bytes) {
        Ok(store) if store.get(TOGGLE_KEY).and_then(Value::as_str) == Some("\"on\"") => Toggle::On,
        Ok(_) => Toggle::Off,
        Err(_) => Toggle::Unknown,
    }
}

/// One press of a row's button, stored by panorama-spaces.
#[derive(Clone, Debug, Deserialize)]
pub(super) struct Click {
    /// The loss the row belonged to.
    pub loss: u64,
    /// The token of the look being confirmed. None: a first click, asking to look.
    pub token: Option<String>,
    pub at: u64,
}

pub(super) fn clicks(home: &Home) -> HashMap<String, Click> {
    let read = || -> Option<HashMap<String, Click>> {
        let file = home.read(TOGGLE_STORE, TOGGLE_LIMIT, 0o600).ok()??;
        let store: HashMap<String, Value> = serde_json::from_slice(&file.bytes).ok()?;
        serde_json::from_str(store.get(CLICKS_KEY)?.as_str()?).ok()
    };
    read().unwrap_or_default()
}
