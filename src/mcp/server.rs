//! The MCP server (rmcp, behind this thin adapter): one deprecated tool, `report_thread`, and
//! no instructions. It speaks what rmcp speaks: the 2025-06-18 `initialize` handshake and
//! Claude Code's 2026-07-28 `server/discover`.

use super::tool::{NAME, tool};
use crate::snapshot::pyjson::{J, dumps};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, DiscoverResult,
    Implementation, InitializeRequestParams, InitializeResult, ListToolsResult,
    PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerConfig, SubscriptionFilter,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler};
use serde_json::Value;

/// The server's name.
pub const SERVER: &str = "herdr-panorama";

/// The server. It holds nothing: no call reads a session, a clock or a file.
pub struct Server;

/// What every `report_thread` call the schema allows answers.
pub fn ignored() -> J {
    J::obj([
        ("status", J::str("ignored")),
        ("reason", J::str("threads were removed")),
    ])
}

/// A tool result as the Python server returns a dict: its JSON as text (indented) and as
/// structured content.
fn result(j: &J) -> CallToolResult {
    let text = dumps(j, 2);
    let structured: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let mut r = CallToolResult::success(vec![ContentBlock::text(text)]);
    r.structured_content = Some(structured);
    r
}

impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        // what the Python server (mcp 2.3) advertises under 2026-07-28: tools, prompts and
        // resources, each with list-changed notifications, resources with subscriptions
        let caps = ServerCapabilities::builder()
            .enable_prompts()
            .enable_prompts_list_changed()
            .enable_resources()
            .enable_resources_list_changed()
            .enable_resources_subscribe()
            .enable_tools()
            .enable_tool_list_changed()
            .build();
        ServerConfig::new(caps).with_server_info(Implementation::new(SERVER, ""))
    }

    /// The 2025-06-18 handshake, as the Python server answers it: the same capabilities with
    /// no list-change notifications and no subscriptions.
    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, ErrorData> {
        context.peer.set_peer_info(request.clone());
        let mut info = self.negotiate_initialize(&request)?;
        let caps = &mut info.capabilities;
        if let Some(t) = caps.tools.as_mut() {
            t.list_changed = Some(false);
        }
        if let Some(p) = caps.prompts.as_mut() {
            p.list_changed = Some(false);
        }
        if let Some(r) = caps.resources.as_mut() {
            r.list_changed = Some(false);
            r.subscribe = Some(false);
        }
        Ok(info)
    }

    /// Discovery (2026-07-28), as the Python server answers it: only that version.
    async fn discover(
        &self,
        _context: RequestContext<RoleServer>,
    ) -> Result<DiscoverResult, ErrorData> {
        Ok(DiscoverResult::from_server_info(
            vec![ProtocolVersion::V_2026_07_28],
            self.get_info(),
        ))
    }

    /// Claude Code listens for list changes (`subscriptions/listen`); nothing ever changes, but
    /// the subscription is acknowledged as the Python server acknowledges it.
    fn accepted_subscription_filter(
        &self,
        requested: &SubscriptionFilter,
    ) -> Option<SubscriptionFilter> {
        Some(requested.clone())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(vec![tool()]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        if request.name != NAME {
            let msg = format!("Unknown tool: {}", request.name);
            return Ok(CallToolResult::error(vec![ContentBlock::text(msg)]).into());
        }
        let args = request.arguments.unwrap_or_default();
        if !args.get("thread_id").is_some_and(Value::is_string) {
            let msg = format!("Error executing tool {NAME}: thread_id: Field required");
            return Ok(CallToolResult::error(vec![ContentBlock::text(msg)]).into());
        }
        Ok(result(&ignored()).into())
    }
}
