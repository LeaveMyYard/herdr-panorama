//! `panorama mcp`: the Claude Code plugin's MCP stdio server (Python `mcp_server.py`), with one
//! tool, `report_thread`, for orchestrators. It writes the explicit thread channel
//! ([`crate::report`]).
//!
//! - [`tool`]: the tool's text and schemas, word for word as the Python server lists them.
//! - [`call`]: one call: checks, cleaning, the merge, the result.
//! - [`session`]: which session a call reports for.
//! - [`server`]: the rmcp adapter.
//!
//! A `report_thread` call waits for the events file's lock (`report::emit`) on the server's one
//! thread, so while another writer holds it the server answers nothing else; Python's server
//! blocks the same way. The lock is held only for one append (and a compaction).
//!
//! Stated deviations: a call the schema refuses (no `thread_id`, a wrong type) says so in its
//! own words, not pydantic's; an events file that cannot be written answers `status: error`
//! instead of a tool error. The protocol envelope (capabilities, the 2026-07-28 cache fields)
//! is rmcp's. JSON-RPC edges no client sends are answered as rmcp answers them: a batch array
//! or a request without `"jsonrpc"` is refused (-32600; Python ignores or answers them),
//! `tools/call` without params and `completion/complete` are -32601 (Python: -32602), and end
//! of input before `initialize` exits 1 with a line on stderr (Python: 0).

pub mod call;
pub mod server;
pub mod session;
pub mod tool;

use herdr_agent_kit::herdr_api::Herdr;
use herdr_agent_kit::plugin_env::PluginEnv;
use rmcp::ServiceExt;
use server::Server;
use session::Sessions;

/// The server for this process's environment.
pub fn server(env: &PluginEnv) -> Server {
    let pane = env
        .pane_id()
        .map(|p| (Herdr::from_env().with_client("panorama"), p.to_owned()));
    Server {
        events_dir: crate::collect::paths::events_dir(env),
        sessions: Sessions {
            pane,
            env: env.var("CLAUDE_CODE_SESSION_ID").map(str::to_owned),
        },
        clock: crate::snapshot::now,
    }
}

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
    let srv = server(&PluginEnv::from_env());
    rt.block_on(async move {
        match srv.serve(rmcp::transport::stdio()).await {
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
