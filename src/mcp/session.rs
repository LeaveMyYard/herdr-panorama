//! The session a `report_thread` call reports for (Python `mcp_server.session_id`), resolved
//! per call: the pane's current Claude session from herdr (`HERDR_PANE_ID`, then `pane get`;
//! it follows /clear), else `CLAUDE_CODE_SESSION_ID`.

use crate::trace::HerdrClient;
use herdr_agent_kit::herdr_api::PaneId;

/// Where the session id comes from.
pub struct Sessions {
    /// herdr, when the server runs in a herdr pane (`HERDR_PANE_ID`).
    pub pane: Option<(HerdrClient, String)>,
    /// `CLAUDE_CODE_SESSION_ID`.
    pub env: Option<String>,
}

impl Sessions {
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
