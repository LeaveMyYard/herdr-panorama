//! One message from a viewer (Python `Core._handle`).

use super::viewer::{Role, Viewer};
use super::{Event, Verify};
use crate::proto::{PROTOCOL, compatible};
use serde_json::{Map, Value, json};
use tokio::sync::{mpsc, oneshot};

/// What the connection does after a message.
#[derive(Debug, PartialEq, Eq)]
pub enum Next {
    Go,
    /// Close the connection (bye, anything before hello).
    Close,
    /// An incompatible hello: say why, then close.
    Refuse(Vec<u8>),
    /// A compatible hello: the welcome (and the last state) are queued; start writing.
    Welcomed,
}

fn str_of(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str).map(str::to_owned)
}

/// A hello's `protocol` as Python's `str(version)` reads it: a string, or a number's text
/// (`1` and `1.3` are compatible there too); anything else has no major.
fn version_text(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn object(v: Option<&Value>) -> Map<String, Value> {
    v.and_then(Value::as_object).cloned().unwrap_or_default()
}

/// Handles `msg` from `v`; `seen` collects a read's worth of seen ids.
pub async fn handle(
    v: &Viewer,
    msg: Map<String, Value>,
    seen: &mut Vec<String>,
    verify: &impl Verify,
    events: &mpsc::UnboundedSender<Event>,
) -> Next {
    if let Some(hello) = msg.get("hello") {
        let hello = object(Some(hello));
        if !version_text(hello.get("protocol")).is_some_and(|v| compatible(&v)) {
            let line = crate::proto::framing::encode(
                &json!({"error": {"message": "protocol", "protocol": PROTOCOL}}),
            );
            return Next::Refuse(line.unwrap_or_default());
        }
        let claim = str_of(hello.get("pane_id"));
        let pane = match (claim, v.pid) {
            (Some(p), Some(pid)) if verify.verify(&p, pid).await => Some(p),
            _ => None,
        };
        let role = if hello.get("role").and_then(Value::as_str) == Some("key") {
            Role::Key
        } else {
            Role::Panel
        };
        v.said_hello(role, pane, str_of(hello.get("tab_id")));
        return Next::Welcomed;
    }
    if !v.hello() {
        return Next::Close; // anything before hello: not a panel of ours
    }
    if let Some(ids) = msg.get("seen") {
        if let Some(ids) = ids.as_array() {
            seen.extend(ids.iter().filter_map(|i| i.as_str().map(str::to_owned)));
        }
        return Next::Go;
    }
    if let Some(req) = msg.get("focusing") {
        // only a verified panel's word counts
        if let (Some(_), Some(pane)) = (v.verified_pane(), str_of(object(Some(req)).get("pane_id")))
            && !pane.is_empty()
        {
            let _ = events.send(Event::Focusing(pane));
        }
        return Next::Go;
    }
    if let Some(req) = msg.get("filter") {
        if let Some(name) = str_of(object(Some(req)).get("name")).filter(|n| !n.is_empty())
            && v.role() == Role::Panel
        {
            let _ = events.send(Event::Filter(name));
        }
        return Next::Go;
    }
    if let Some(req) = msg.get("open") {
        let (reply, answer) = oneshot::channel();
        let did = if events
            .send(Event::Open {
                req: object(Some(req)),
                reply,
            })
            .is_ok()
        {
            answer.await.unwrap_or_else(|_| "failed".into()) // a broken request never takes the core down
        } else {
            "failed".into()
        };
        if let Ok(line) = crate::proto::framing::encode(&json!({"opened": {"result": did}})) {
            v.send(std::sync::Arc::new(line));
        }
        return Next::Go;
    }
    if msg.contains_key("bye") {
        return Next::Close;
    }
    Next::Go // unknown: ignored (a newer minor)
}
