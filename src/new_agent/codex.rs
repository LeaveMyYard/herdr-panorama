//! Codex starts with an embedded server so hook ownership can be proved from
//! its pane process. The pinned kit has no start-args API yet; retain its parser,
//! request timeout and readiness polling while adding this supported argument.

use herdr_agent_kit::herdr_api::{Herdr, HerdrError, PaneId, START_POLL, Sanitized, calls};
use serde_json::json;
use std::time::Duration;

pub(super) async fn start(
    herdr: &Herdr,
    name: &str,
    pane: &str,
    timeout_ms: u32,
) -> Result<(), HerdrError> {
    let name = Sanitized::new(name);
    let pane = PaneId::new(pane);
    let request = calls::start_agent(&name, "codex", &pane, timeout_ms);
    let mut params = request.params().clone();
    params["args"] = json!(["--no-daemon"]);
    let deadline = tokio::time::Instant::now() + Duration::from_millis(u64::from(timeout_ms));
    let client = herdr
        .clone()
        .with_timeout(request.timeout().expect("start has its own timeout"));
    let mut agent = request.parse(client.raw("agent.start", params).await)?;
    loop {
        if agent.pane_id.as_ref() != Some(&pane) {
            return Err(HerdrError::Garbled(
                "Codex start returned another pane".into(),
            ));
        }
        if agent.is_ready() {
            return Ok(());
        }
        if tokio::time::Instant::now() + START_POLL > deadline {
            return Err(HerdrError::NoReply(format!(
                "the agent was not ready within {timeout_ms} ms"
            )));
        }
        tokio::time::sleep(START_POLL).await;
        if let Some(polled) = herdr.agent_named(name.as_str()).await? {
            agent = polled;
        }
    }
}

#[cfg(test)]
mod tests;
