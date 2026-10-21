//! The open key's question to the core (Python `panel.request_open`): a `key` connection says
//! hello and `open`, and the core answers `opened` with what it did.

use crate::proto::PROTOCOL;
use crate::proto::framing::{LineReader, encode};
use crate::proto::socket::connect;
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// How long the key waits for the core (Python's 8 s).
pub const OPEN_WAIT: Duration = Duration::from_secs(8);

/// What the core did (`panel`, `home`, or why neither; `failed` for an answer without a
/// result); `None` when there is no core of ours at `path` (none listening, another user's,
/// an error, no answer within `wait`).
pub async fn request_open(path: &Path, uid: u32, wait: Duration) -> Option<String> {
    let mut s = tokio::time::timeout(wait, connect(path, uid))
        .await
        .ok()?
        .ok()?;
    let ask = async {
        let hello = json!({"hello": {"protocol": PROTOCOL, "role": "key"}});
        for m in [hello, json!({"open": {}})] {
            s.write_all(&encode(&m).ok()?).await.ok()?;
        }
        let (mut reader, mut buf) = (LineReader::default(), vec![0u8; 65536]);
        loop {
            let n = s.read(&mut buf).await.ok()?;
            if n == 0 {
                return None;
            }
            for msg in reader.feed(&buf[..n]).ok()? {
                if let Some(opened) = msg.get("opened").and_then(|o| o.as_object()) {
                    let did = opened
                        .get("result")
                        .and_then(|r| r.as_str())
                        .filter(|r| !r.is_empty());
                    return Some(did.unwrap_or("failed").to_owned());
                }
                if msg.contains_key("error") {
                    return None;
                }
            }
        }
    };
    let did = tokio::time::timeout(wait, ask).await.ok().flatten();
    if let Ok(bye) = encode(&json!({"bye": {}})) {
        let _ = tokio::time::timeout(Duration::from_millis(200), s.write_all(&bye)).await;
    }
    did
}
