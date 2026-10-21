//! The core's full snapshot for the panels (Python `protocol.state_message`; protocol 1.3):
//! the model, the unseen marks, herdr's focused pane, the visible tab, each tab's panel and
//! the filter every view shows. Always whole, never a delta.

use panorama_core::model::Swarm;
use panorama_core::view::Marks;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// What the home view publishes besides its model.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Published {
    /// herdr's last focused pane.
    pub focused_pane: Option<String>,
    /// The tab herdr shows (`None`: unknown, a viewer then counts itself visible).
    pub visible_tab: Option<String>,
    /// tab → its panel's pane (where a switch to that tab puts the keyboard).
    pub panels: BTreeMap<String, String>,
}

/// `{"state": …}` for `swarm` with its `unseen` marks and `filter`'s name.
pub fn state_message(swarm: &Swarm, unseen: &Marks, p: &Published, filter: Option<&str>) -> Value {
    let mut st = json!({
        "protocol": crate::proto::PROTOCOL,
        "swarm": swarm,
        "unseen": unseen,
        "focused_pane": p.focused_pane,
        "visible_tabs": p.visible_tab.as_ref().map(|t| vec![t]),
        "panels": p.panels,
    });
    if let Some(f) = filter {
        st["filter"] = json!(f);
    }
    json!({ "state": st })
}

#[cfg(test)]
mod tests;
