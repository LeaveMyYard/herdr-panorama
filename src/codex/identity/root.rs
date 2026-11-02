//! Fresh metadata-only root proof, never a cached history observation.
use super::valid_thread_id;
use crate::codex::{Options, protocol::Client};
use serde_json::{Value, json};

pub(super) fn is_root(value: &Value, expected: &str) -> bool {
    let Some(thread) = value.get("thread") else {
        return false;
    };
    thread.get("id").and_then(Value::as_str) == Some(expected)
        && thread.get("parentThreadId") == Some(&Value::Null)
        && matches!(
            thread.get("source").and_then(Value::as_str),
            Some("cli" | "vscode" | "exec" | "appServer")
        )
}

pub(super) async fn verify(options: &Options, id: &str) -> Result<(), &'static str> {
    if !valid_thread_id(id) {
        return Err("thread ID must be a canonical lower-case UUID");
    }
    let mut client = Client::spawn(options).map_err(|_| "could not start Codex metadata reader")?;
    let result = tokio::time::timeout(options.timeout, async {
        client
            .initialize()
            .await
            .map_err(|_| "Codex initialization unavailable")?;
        let value = client
            .call("thread/read", json!({"threadId":id,"includeTurns":false}))
            .await
            .map_err(|_| "thread metadata unavailable; retry after Codex persists the thread")?;
        if is_root(&value, id) {
            Ok(())
        } else {
            Err("metadata does not prove this exact thread is a root")
        }
    })
    .await
    .unwrap_or(Err("Codex metadata read timed out"));
    client.stop().await;
    result
}
