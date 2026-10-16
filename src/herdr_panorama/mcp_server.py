"""MCP stdio server (Claude Code plugin): ``report_thread`` for orchestrators.

Writes the explicit thread channel (events.py), the same format ``panorama emit``
uses. The session is resolved per call: the pane's current Claude session from
herdr (``HERDR_PANE_ID`` -> ``herdr pane get``; it follows /clear), else
``CLAUDE_CODE_SESSION_ID``.
"""

from __future__ import annotations

import os
from typing import Any

try:  # mcp 2.x renamed FastMCP to MCPServer; the decorator API is the same
    from mcp.server.mcpserver import MCPServer
except ImportError:  # pragma: no cover - mcp 1.x
    from mcp.server.fastmcp import FastMCP as MCPServer

from . import events, herdr

INSTRUCTIONS = "Report multi-step work to the herdr swarm view with report_thread."

DESCRIPTION = """Declare or update a thread of work for the herdr swarm view (herdr-panorama).

Call it when multi-step work starts (a PR through review rounds, a plan's tasks, several
subagents on one goal) and whenever its phase changes. Returns immediately.
- thread_id: stable id, e.g. "pr-595" or "task-3"; title: short name, e.g. "PR #595 ledger".
- phase: the current phase ("review r2"); phases_done: finished phases in order;
  next: the phases still to come. Omitted lists keep what you reported before.
- status: "active" (default), "done", "failed" or "cancelled" when the thread ends.
- agents: optional subagent ids or descriptions (a trailing * matches a prefix) that work on it."""


_INVISIBLE = dict.fromkeys(map(ord, "\u200e\u200f\u061c\u202a\u202b\u202c\u202d\u202e\u2066\u2067\u2068\u2069"))


def _text(value, limit: int = events.MAX_TEXT):
    """Control characters (clean) and bidi/format characters stripped, length capped."""
    if not isinstance(value, str):
        return value
    return herdr.clean(value, limit).translate(_INVISIBLE)


def session_id() -> str | None:
    pane = os.environ.get("HERDR_PANE_ID")
    if pane:
        res, _ = herdr.call("pane", "get", pane)
        value = (((res or {}).get("pane") or {}).get("agent_session") or {}).get("value")
        if isinstance(value, str) and value:
            return value
    return os.environ.get("CLAUDE_CODE_SESSION_ID") or None


mcp = MCPServer("herdr-panorama", instructions=INSTRUCTIONS)


@mcp.tool(description=DESCRIPTION)
def report_thread(thread_id: str, title: str | None = None, phase: str | None = None,
                  phases_done: list[str] | None = None, next: list[str] | None = None,  # noqa: A002
                  status: str | None = None, agents: list[str] | None = None) -> dict[str, Any]:
    sid = session_id()
    if not sid:
        return {"status": "unavailable", "message": "No Claude session id; nothing recorded."}
    if status is not None and status not in events.STATUSES:
        return {"status": "error", "message": f"status must be one of {sorted(events.STATUSES)}"}
    for name, items in (("phases_done", phases_done), ("next", next), ("agents", agents)):
        if items is not None and len(items) > events.MAX_ITEMS:
            return {"status": "error", "message": f"{name}: at most {events.MAX_ITEMS} items"}
    for a in agents or []:
        if isinstance(a, str) and a.endswith("*") and len(a.rstrip("*")) < events.MIN_PREFIX:
            return {"status": "error",
                    "message": f"agent pattern {a!r}: name at least {events.MIN_PREFIX} characters before *"}
    out = events.report(sid, _text(thread_id), _text(title), _text(phase),
                        [_text(p) for p in phases_done] if phases_done is not None else None,
                        [_text(p) for p in next] if next is not None else None,
                        status, [_text(a) for a in agents] if agents else None)
    return {"status": "recorded", **out}


def run() -> None:
    mcp.run()
