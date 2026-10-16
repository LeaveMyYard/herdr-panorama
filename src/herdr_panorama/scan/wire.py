"""Lenient parsing of Claude Code transcript records.

Follows zoetrope ``src/provider/claude/wire.rs`` and ``mod.rs`` (MIT): unknown
record types and content blocks are skipped, every field is optional,
``message.content`` may be a string or a list, ``toolUseResult`` a string or an
object, a blank or unparsable line states nothing and never raises. A record
without a ``timestamp`` inherits the previous one in the same file.

Changes from zoetrope (study §6): ``<task-notification>`` is read from
``user`` strings, ``queue-operation`` ``enqueue`` and ``attachment``
``queued_command`` in every file (not only main-file user strings), ``killed``
is a terminal status, and ``SendMessage`` results with ``resumedAgentId`` are
resumes.
"""

from __future__ import annotations

import json
import re
from bisect import bisect_right
from datetime import datetime

SPAWN_TOOLS = ("Agent", "Task")
NOTICE_STATUS = {"completed": "done", "failed": "fail", "error": "fail", "killed": "cxl", "stopped": "cxl"}
SUMMARY_MAX = 200

# Lines worth a full JSON parse in a full-file pass (the tail is parsed separately).
MARKERS = (
    b'"name":"Agent"', b'"name":"Task"', b'"name":"SendMessage"', b'"name":"Workflow"',
    b"async_launched", b'"totalToolUseCount"', b"resumedAgentId", b"runId", b"task-notification",
    b"ask_operator", b"Recorded as Q", b"already with the operator",
    b'"type":"custom-title"', b'"type":"agent-name"', b'"type":"ai-title"',
    # background work (design turn 8): Bash in the background, Monitor watches, TaskStop / KillShell
    b'"run_in_background":true', b'"backgroundTaskId"', b'"name":"Monitor"', b'"timeoutMs"',
    b'"name":"TaskStop"', b'"name":"KillShell"', b'"task_type"', b'"shell_id"',
)
BG_STOP_TOOLS = ("TaskStop", "KillShell")
ORPHAN = "__orphan_summary__"  # Claude Code's marker id in the notice that lists tasks a restart orphaned
PROMPT_MARKERS = (b'"role":"user"', b'"content":"')  # a user message with string content, any key order

_TAG = re.compile(r"<(task-id|tool-use-id|status|tool_uses)>\s*([^<]*?)\s*</\1>")
_TASK_IDS = re.compile(r"<task-id>\s*([^<]*?)\s*</task-id>")
_EVENT = re.compile(r"<event>\s*\[Monitor expired")  # a Monitor's last event: it ended (no status tag)
_QID = re.compile(r"(?:Recorded as Q|\bQ)(\d+)\b")
_QID_JSON = re.compile(r'"id"\s*:\s*(\d+)\s*,\s*"status"\s*:\s*"(?:open|already_open)"')


def wanted(raw: bytes) -> bool:
    return any(m in raw for m in MARKERS) or all(m in raw for m in PROMPT_MARKERS)


def wanted_lines(lines: list[bytes]) -> list[int]:
    """Indexes of the lines ``wanted`` would accept, found with one C-level search per
    marker over the joined buffer instead of one per marker per line."""
    if not lines:
        return []
    blob = b"\n".join(lines)
    starts, pos = [], 0
    for ln in lines:
        starts.append(pos)
        pos += len(ln) + 1
    hits: set[int] = set()

    def scan(marker: bytes, out: set[int]) -> None:
        i = blob.find(marker)
        while i >= 0:
            idx = bisect_right(starts, i) - 1
            out.add(idx)
            nxt = starts[idx + 1] if idx + 1 < len(starts) else len(blob)
            i = blob.find(marker, nxt)

    for m in MARKERS:
        scan(m, hits)
    users: set[int] = set()
    scan(PROMPT_MARKERS[0], users)
    hits.update(i for i in users if PROMPT_MARKERS[1] in lines[i])
    return sorted(hits)


def parse_line(line: str | bytes) -> dict | None:
    try:
        rec = json.loads(line)
    except (ValueError, RecursionError, UnicodeDecodeError):
        return None
    return rec if isinstance(rec, dict) else None


def ts(value) -> float | None:
    if value is None or isinstance(value, (bool, list, dict)):
        return None
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        return value / 1000.0 if value > 1e11 else float(value)
    try:
        return datetime.fromisoformat(str(value).replace("Z", "+00:00")).timestamp()
    except (ValueError, TypeError, OverflowError):
        try:
            return ts(int(value))
        except (ValueError, TypeError, OverflowError):
            return None


def blocks(rec: dict) -> list[dict]:
    content = (rec.get("message") or {}).get("content") if isinstance(rec.get("message"), dict) else None
    return [b for b in content if isinstance(b, dict)] if isinstance(content, list) else []


def string_content(rec: dict) -> str | None:
    msg = rec.get("message")
    content = msg.get("content") if isinstance(msg, dict) else None
    return content if isinstance(content, str) and content.strip() else None


def text_of(content) -> str:
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return " ".join(str(b.get("text") or "") for b in content if isinstance(b, dict))
    return ""


_BLOCK = re.compile(r"<task-notification>.*?</task-notification>", re.S)


def parse_notices(text: str) -> list[dict]:
    """Every ``<task-notification>`` block in a carrier (one carrier can hold several), one
    notice per task it names: the notice a resumed session gets for the background tasks its
    previous process left lists several ``<task-id>``s (plus an internal marker id, skipped)."""
    if not isinstance(text, str) or not text.lstrip().startswith("<task-notification>"):
        return []
    out = []
    for m in _BLOCK.finditer(text):
        n = parse_notice(m.group(0))
        if n is None:
            continue
        ids = [i for i in _TASK_IDS.findall(m.group(0)) if i and not i.startswith(ORPHAN)]
        for i in ids or [n["task"]]:
            out.append({**n, "task": i})
    return out


def parse_notice(text: str) -> dict | None:
    """One ``<task-notification>`` block -> {task, tool_use, status, tool_uses}; status None if unknown."""
    if not isinstance(text, str) or not text.lstrip().startswith("<task-notification>"):
        return None
    tags = dict(_TAG.findall(text))
    first = next((i for i in _TASK_IDS.findall(text) if i and not i.startswith(ORPHAN)), None)
    if not first:
        return None
    uses = tags.get("tool_uses", "")
    status = NOTICE_STATUS.get(tags.get("status", ""))
    if status is None and _EVENT.search(text):
        status = "done"  # "[Monitor expired after 30m ...]": the watch is over
    return {"task": first, "tool_use": tags.get("tool-use-id"),
            "status": status, "tool_uses": int(uses) if uses.isdigit() else None}


def notice_text(rec: dict) -> str | None:
    """The ``<task-notification>`` a record carries, from any of its three carriers."""
    kind = rec.get("type")
    if kind == "queue-operation":
        return rec.get("content") if rec.get("operation") == "enqueue" else None
    if kind == "attachment":
        att = rec.get("attachment")
        return att.get("prompt") if isinstance(att, dict) and att.get("type") == "queued_command" else None
    if kind == "user":
        return string_content(rec)
    return None


def is_human_prompt(rec: dict) -> bool:
    """zoetrope ``is_human_prompt``: ``origin.kind == "human"``; legacy records without an origin:
    any string that is not a task notification. Meta records are not prompts."""
    if rec.get("type") != "user" or rec.get("isMeta"):
        return False
    text = string_content(rec)
    if text is None:
        return False
    origin = rec.get("origin")
    kind = origin.get("kind") if isinstance(origin, dict) else None
    if kind is not None:
        return kind == "human"
    return not text.lstrip().startswith(("<task-notification>", "<local-command", "<command-"))


def question_ids(text: str) -> set[int]:
    ids = {int(m) for m in _QID.findall(text)}
    ids.update(int(m) for m in _QID_JSON.findall(text))
    return ids


def truncate_summary(s: str) -> str:
    flat = " ".join(str(s).split())
    return flat if len(flat) <= SUMMARY_MAX else flat[: SUMMARY_MAX - 1] + "…"


def short_path(path: str, cwd: str | None) -> str:
    """zoetrope ``short_path``: relative to cwd at a path-component boundary, else the tail."""
    rel = path
    if cwd and path.startswith(cwd):
        rest = path[len(cwd):]
        if (rest.startswith("/") or cwd.endswith("/")) and rest.strip("/"):
            rel = rest.lstrip("/")
    return rel if len(rel) <= SUMMARY_MAX else "…" + rel[-(SUMMARY_MAX - 1):]


def summarize_tool(name: str, inp, cwd: str | None = None) -> str:
    """``Name(one-line summary)``, after zoetrope ``summarize_tool``. Bash prefers the
    human ``description`` over the raw command (the cards are narrow)."""
    short = name.split("__")[-1] if name.startswith("mcp__") else name
    if not isinstance(inp, dict):
        return short

    def pick(*keys):
        for k in keys:
            v = inp.get(k)
            if isinstance(v, str) and v.strip():
                return v
        return None

    if name == "Bash":
        s = pick("description", "command")
    elif name in ("Read", "Write", "Edit", "NotebookEdit"):
        p = pick("file_path", "path", "notebook_path")
        s = short_path(p, cwd) if p else None
    elif name in SPAWN_TOOLS:
        s = pick("description", "subagent_type")
    elif name == "WebFetch":
        s = pick("url")
    elif name in ("ToolSearch", "WebSearch", "Grep", "Glob"):
        s = pick("query", "pattern")
    elif name == "SendMessage":
        s = pick("to")
    else:
        s = pick("description", "query", "command", "file_path", "pattern", "url", "to")
    if not s:
        return short
    first = next((ln for ln in s.splitlines() if ln.strip()), s)
    return f"{short}({truncate_summary(first)[:80]})"
