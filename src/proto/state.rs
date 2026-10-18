//! The core's `state` snapshot (`docs/protocol/panel-v1.md`, "`state`"), read as Python's
//! panel reads it (`PanelApp._got_state`): each field on its own, a field of the wrong shape
//! taken as absent. Only the swarm is strict: a swarm that does not read (see
//! `panorama_core::model::swarm`) rejects the state, and the panel keeps its last picture.

use panorama_core::model::Swarm;
use panorama_core::view::{Filter, Marks, Unseen};
use serde_json::{Map, Value};
use std::collections::HashMap;

/// One snapshot from the core.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub swarm: Swarm,
    /// The unseen marks (an unknown event is dropped).
    pub unseen: Marks,
    /// The pane herdr last reported focused.
    pub focused_pane: Option<String>,
    /// The tabs herdr shows; `None`: unknown (a viewer then counts itself visible).
    pub visible_tabs: Option<Vec<String>>,
    /// Tab id → its per-tab panel's pane (1.1).
    pub panels: HashMap<String, String>,
    /// The filter every view shows (1.3); `None`: an older core, or an unknown name.
    pub filter: Option<Filter>,
}

fn strings(v: &Value) -> impl Iterator<Item = String> + '_ {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str().map(str::to_owned))
}

impl State {
    /// The snapshot in `st`, or why it does not read.
    pub fn read(st: &Map<String, Value>) -> Result<Self, serde_json::Error> {
        let swarm = match st.get("swarm") {
            Some(Value::Null) | None => Swarm::default(),
            Some(v) => serde_json::from_value(v.clone())?,
        };
        let unseen = st
            .get("unseen")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(id, ev)| {
                Some((
                    id.clone(),
                    serde_json::from_value::<Unseen>(ev.clone()).ok()?,
                ))
            })
            .collect();
        let panels = st
            .get("panels")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(tab, pane)| Some((tab.clone(), pane.as_str()?.to_owned())))
            .collect();
        Ok(Self {
            swarm,
            unseen,
            focused_pane: st
                .get("focused_pane")
                .and_then(Value::as_str)
                .map(str::to_owned),
            visible_tabs: st
                .get("visible_tabs")
                .filter(|v| v.is_array())
                .map(|v| strings(v).collect()),
            panels,
            filter: st
                .get("filter")
                .and_then(Value::as_str)
                .and_then(Filter::from_name),
        })
    }
}

#[cfg(test)]
mod tests;
