//! One message from a viewer (Python `Core._handle`).

use super::viewer::{Role, Viewer};
use super::{Event, Verify};
use crate::proto::{PROTOCOL, compatible};
use serde_json::{Map, Value, json};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

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

/// Hands `ev` to the app, waiting while the channel is full (that slows this viewer's reader
/// down, as Python's reader thread is); false when the app is gone or the viewer closed.
pub async fn emit(v: &Viewer, events: &mpsc::Sender<Event>, ev: Event) -> bool {
    tokio::select! {
        r = events.send(ev) => r.is_ok(),
        () = v.closed() => false,
    }
}

/// Handles `msg` from `v`; `seen` collects a read's worth of seen ids. A pane check that has
/// not answered by `verify_by` (the hello deadline) reads as unverified.
pub async fn handle(
    v: &Viewer,
    msg: Map<String, Value>,
    seen: &mut Vec<String>,
    verify: &impl Verify,
    verify_by: Instant,
    events: &mpsc::Sender<Event>,
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
        // a pid of 0 (a peer in another pid namespace) is never checked, as Python's `v.pid and`
        let pane = match (claim, v.pid.filter(|pid| *pid > 0)) {
            (Some(p), Some(pid))
                if tokio::time::timeout_at(verify_by, verify.verify(&p, pid))
                    .await
                    .unwrap_or(false) =>
            {
                Some(p)
            }
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
            emit(v, events, Event::Focusing(pane)).await;
        }
        return Next::Go;
    }
    if let Some(req) = msg.get("filter") {
        if let Some(name) = str_of(object(Some(req)).get("name")).filter(|n| !n.is_empty())
            && v.role() == Role::Panel
        {
            emit(v, events, Event::Filter(name)).await;
        }
        return Next::Go;
    }
    if let Some(req) = msg.get("open") {
        let (reply, answer) = oneshot::channel();
        let open = Event::Open {
            req: object(Some(req)),
            reply,
        };
        let did = if emit(v, events, open).await {
            // a broken request never takes the core down; a closed viewer stops waiting
            tokio::select! {
                a = answer => a.unwrap_or_else(|_| "failed".into()),
                () = v.closed() => return Next::Close,
            }
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

#[cfg(test)]
mod tests;
