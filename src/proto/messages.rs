//! The messages (`docs/protocol/panel-v1.md`, "Messages"): what a panel says to the core,
//! and how it reads the core's answers.

use super::PROTOCOL;
use serde_json::{Map, Value, json};

/// Viewer → core.
#[derive(Clone, Debug, PartialEq)]
pub enum ToCore {
    /// The first message: this panel's claimed pane and tab (`null` when unknown).
    Hello {
        pane_id: Option<String>,
        tab_id: Option<String>,
    },
    /// The operator looked at these agents.
    Seen(Vec<String>),
    /// A switch is about to put the keyboard on this pane (1.2).
    Focusing(String),
    /// The operator chose this filter here (1.3).
    Filter(String),
    /// The viewer is closing.
    Bye,
}

impl ToCore {
    /// As a protocol message.
    pub fn to_json(&self) -> Value {
        match self {
            Self::Hello { pane_id, tab_id } => json!({"hello": {
                "protocol": PROTOCOL, "role": "panel", "pane_id": pane_id, "tab_id": tab_id,
            }}),
            Self::Seen(ids) => json!({ "seen": ids }),
            Self::Focusing(pane) => json!({"focusing": {"pane_id": pane}}),
            Self::Filter(name) => json!({"filter": {"name": name}}),
            Self::Bye => json!({"bye": {}}),
        }
    }
}

/// Core → viewer, as a panel reads it (Python `PanelClient._session`).
#[derive(Clone, Debug, PartialEq)]
pub enum FromCore {
    /// The answer to a compatible hello.
    Welcome,
    /// An incompatible hello: why (the connection then closes).
    Error(String),
    /// A full snapshot, still to be read (`state::State::read`).
    State(Map<String, Value>),
    /// The panels go: the viewer exits.
    Shutdown,
    /// Anything else (`ping`, an unknown message): ignored.
    Other,
}

impl FromCore {
    /// What a decoded line says. A message is told by its key, in Python's order: `welcome`,
    /// `error`, `state` (an object), `shutdown`.
    pub fn read(msg: Map<String, Value>) -> Self {
        if msg.contains_key("welcome") {
            return Self::Welcome;
        }
        if let Some(err) = msg.get("error") {
            let text = err
                .get("message")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty());
            return Self::Error(text.unwrap_or("refused").to_owned());
        }
        match msg.get("state") {
            Some(Value::Object(st)) => return Self::State(st.clone()),
            Some(_) => return Self::Other,
            None => {}
        }
        if msg.contains_key("shutdown") {
            Self::Shutdown
        } else {
            Self::Other
        }
    }
}

#[cfg(test)]
mod tests;
