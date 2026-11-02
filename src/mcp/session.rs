//! The session a `report_thread` call reports for (Python `mcp_server.session_id`), resolved
//! per call: the pane's current Claude session from herdr (`HERDR_PANE_ID`, then `pane get`;
//! it follows /clear), else `CLAUDE_CODE_SESSION_ID`.

use herdr_agent_kit::herdr_api::{Herdr, PaneId};
use serde_json::{Map, Value};

/// Where the session id comes from.
pub struct Sessions {
    /// herdr, when the server runs in a herdr pane (`HERDR_PANE_ID`).
    pub pane: Option<(Herdr, String)>,
    /// `CLAUDE_CODE_SESSION_ID`.
    pub env: Option<String>,
}

impl Sessions {
    /// Codex sends exact caller identity on every request, including child calls on
    /// a shared MCP process. Invalid present metadata must never fall back to a pane.
    pub async fn for_call(
        &self,
        meta: &Map<String, Value>,
    ) -> Result<Option<String>, &'static str> {
        match meta.get("threadId") {
            Some(Value::String(id)) if crate::codex::identity::valid_thread_id(id) => {
                Ok(Some(id.clone()))
            }
            Some(_) => Err("Invalid Codex threadId; nothing recorded."),
            None => Ok(self.current().await),
        }
    }

    /// The session id now, if any.
    pub async fn current(&self) -> Option<String> {
        if let Some((herdr, pane)) = &self.pane
            && let Ok(info) = herdr.pane(&PaneId::new(pane.clone())).await
            && let Some(v) = info.agent_session.as_ref().and_then(|s| s.value())
        {
            return Some(v.to_owned());
        }
        self.env.clone().filter(|s| !s.is_empty())
    }
}

#[cfg(test)]
mod tests;
