"""MCP stdio server (Claude Code plugin): ``report_thread``, deprecated.

Threads were removed (plan ``uniform-agent-tree.md`` section 6): the tool is still listed with
its old schema, so a session that was told to call it never sees "Unknown tool", but a call
records nothing and answers ``ignored``. The server has no instructions.
"""

from __future__ import annotations

from typing import Any

try:  # mcp 2.x renamed FastMCP to MCPServer; the decorator API is the same
    from mcp.server.mcpserver import MCPServer
except ImportError:  # pragma: no cover - mcp 1.x
    from mcp.server.fastmcp import FastMCP as MCPServer

DESCRIPTION = """Deprecated: has no effect. herdr-panorama no longer has threads: it shows every
agent and subagent under the name its spawner gave it, so there is nothing to report.
Calls are accepted and ignored."""

IGNORED = {"status": "ignored", "reason": "threads were removed"}

mcp = MCPServer("herdr-panorama")


@mcp.tool(description=DESCRIPTION)
def report_thread(thread_id: str, title: str | None = None, phase: str | None = None,
                  phases_done: list[str] | None = None, next: list[str] | None = None,  # noqa: A002
                  status: str | None = None, agents: list[str] | None = None) -> dict[str, Any]:
    return dict(IGNORED)


def run() -> None:
    mcp.run()
