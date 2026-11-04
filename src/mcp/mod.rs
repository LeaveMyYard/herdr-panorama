//! `panorama mcp`: the Claude Code plugin's MCP stdio server (Python `mcp_server.py`). Threads
//! were removed (plan `uniform-agent-tree.md` §6), so it has no instructions and one deprecated
//! tool, `report_thread`: still listed with its old schema, so a session that was told to call
//! it never sees "Unknown tool", but every call answers `ignored` and writes nothing. No
//! session is resolved and herdr is never asked. Release N+1 removes the server.
//!
//! - [`tool`]: the tool's text and schemas, word for word as the Python server lists them.
//! - [`server`]: the rmcp adapter and the call's answer.
//!
//! Stated deviations: a call without a string `thread_id` (the schema's one required field) is
//! refused in its own words, not pydantic's; a call whose optional arguments have the wrong
//! type is ignored like any other (pydantic refuses it). The protocol envelope (capabilities,
//! the 2026-07-28 cache fields) is rmcp's. JSON-RPC edges no client sends are answered as rmcp
//! answers them: a batch array or a request without `"jsonrpc"` is refused (-32600; Python
//! ignores or answers them), `tools/call` without params and `completion/complete` are -32601
//! (Python: -32602), and end of input before `initialize` exits 1 with a line on stderr
//! (Python: 0).

pub mod server;
pub mod tool;

use rmcp::ServiceExt;
use server::Server;

/// `panorama mcp` on stdin and stdout, until the client goes; the exit status.
pub fn run() -> i32 {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("panorama mcp: {e}");
            return 1;
        }
    };
    rt.block_on(async move {
        match Server.serve(rmcp::transport::stdio()).await {
            Ok(running) => {
                let _ = running.waiting().await;
                0
            }
            Err(e) => {
                eprintln!("panorama mcp: {e}");
                1
            }
        }
    })
}
