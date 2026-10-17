"""Capture anonymised record templates from real Claude Code / herdr files.

Run on a machine with real transcripts (not in CI):

    uv run python tests/fixtures/capture.py <projects-dir> > tests/fixtures/templates.json

For every record kind herdr-panorama reads, the first real record of that kind
is taken and anonymised: keys, nesting, value types and the structural values
the parser keys on (record ``type``, ``operation``, tool ``name``, status words,
the ``<task-notification>`` tag skeleton) are kept; every id, timestamp, path
and piece of free text is replaced. The test scenarios in conftest.py are
built only from these templates, so the wire format is captured, never
invented.
"""

from __future__ import annotations

import glob
import json
import os
import re
import sys

KEEP_STR = {"type", "role", "agentType", "isFork", "operation", "subtype", "userType", "entrypoint", "status", "agentType",
            "requestShape", "renderedRole", "thinkingDisplay", "permissionMode", "kind", "stop_reason",
            "decision", "source", "reasonType", "taskType", "effort", "perTurnEffort", "service_tier",
            "promptSource", "turnOrigin", "turnPosition", "inference_geo", "mode"}
TOOL_NAMES = {"Agent", "Task", "SendMessage", "Workflow", "AskUserQuestion", "Bash", "Read", "SubagentHandback",
              "StructuredOutput", "mcp__plugin_herdr-inbox_herdr-inbox__ask_operator", "Monitor", "TaskStop", "KillShell"}
UUID = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
TS = re.compile(r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(\.\d+)?Z$")
NOTE_TAGS = re.compile(r"<(task-id|tool-use-id|output-file|status|summary|note|result|usage|subagent_tokens|tool_uses|duration_ms|event)>")
# notice values the parser keys on: Claude Code's marker id for orphaned tasks, a Monitor's expiry event
KEEP_NOTE = {"task-id": ("__orphan_summary__",), "event": ("[Monitor expired",)}


ID_KEY = re.compile(r"^(toolu_|call_)|^[0-9a-f]{8}-[0-9a-f]{4}-|^a?[0-9a-f]{16,}$")


def anon_key(k: str, i: int) -> str:
    """Dictionary keys can be ids too (``wireToolInputs`` is keyed by real tool_use ids)."""
    if not ID_KEY.search(k):
        return k
    return "toolu_KEY" if k.startswith(("toolu_", "call_")) else f"KEY{i}"


def anon(value, key: str | None = None, parent: dict | None = None):
    if isinstance(value, dict):
        return {anon_key(k, i): anon(v, k, value) for i, (k, v) in enumerate(value.items())}
    if isinstance(value, list):
        # keep one element per distinct block type
        seen, out = set(), []
        for v in value:
            t = v.get("type") if isinstance(v, dict) else type(v).__name__
            if t in seen:
                continue
            seen.add(t)
            out.append(anon(v, key, parent))
        return out
    if isinstance(value, bool) or value is None:
        return value
    if isinstance(value, (int, float)):
        return 0 if key and ("Ms" in key or "token" in key.lower() or "At" in key or "time" in key.lower()
                             or any(w in key.lower() for w in ("cost", "duration", "lines", "requests"))) else value
    s = str(value)
    if key in KEEP_STR:
        return s
    if key == "name" and s in TOOL_NAMES:
        return s
    if key == "model" or key == "requestedModel" or key == "resolvedModel" or key == "defaultModel":
        return "claude-model" if s.startswith("claude") else "opus"
    if TS.match(s):
        return "2026-01-01T00:00:00.000Z"
    if UUID.match(s):
        return "00000000-0000-4000-8000-000000000000"
    if s.startswith("toolu_"):
        return "toolu_TEMPLATE"
    if s.lstrip().startswith("<task-notification>"):
        out = []
        for m in NOTE_TAGS.finditer(s):  # each tag in order; its value only where the parser keys on it
            t = m.group(1)
            value = s[m.end(): s.find(f"</{t}>", m.end())]
            keep = next((k for k in KEEP_NOTE.get(t, ()) if value.startswith(k)), "")
            out.append(f"<{t}>{keep}…</{t}>")
        return "<task-notification>" + "".join(out) + "</task-notification>"
    return "" if s else s


def first(path: str, pred) -> dict | None:
    try:
        fh = open(path, encoding="utf-8", errors="replace")
    except OSError:
        return None
    with fh:
        for line in fh:
            try:
                rec = json.loads(line)
            except ValueError:
                continue
            if isinstance(rec, dict) and pred(rec):
                return rec
    return None


def blocks(rec, btype=None, name=None):
    c = (rec.get("message") or {}).get("content")
    if not isinstance(c, list):
        return []
    return [b for b in c if isinstance(b, dict) and (btype is None or b.get("type") == btype)
            and (name is None or b.get("name") == name)]


def tur(rec, key, val=None):
    t = rec.get("toolUseResult")
    return isinstance(t, dict) and key in t and (val is None or t.get(key) == val)


def main() -> None:
    root = sys.argv[1]
    merge = sys.argv[2] if len(sys.argv) > 2 else None  # an existing templates.json: keep its entries, add new kinds
    mains = sorted(glob.glob(os.path.join(root, "*", "*.jsonl")), key=os.path.getsize, reverse=True)[:40]
    subs = sorted(glob.glob(os.path.join(root, "*", "*", "subagents", "agent-*.jsonl")), key=os.path.getsize)[:400]
    wf_journals = glob.glob(os.path.join(root, "*", "*", "subagents", "workflows", "*", "journal.jsonl"))
    wf_summaries = glob.glob(os.path.join(root, "*", "*", "workflows", "wf_*.json"))
    preds = {
        "user_prompt": lambda r: r.get("type") == "user" and isinstance((r.get("message") or {}).get("content"), str)
        and not r.get("isMeta") and not r.get("origin") and not r["message"]["content"].startswith("<"),
        "assistant_text": lambda r: r.get("type") == "assistant" and bool(blocks(r, "text")),
        "assistant_agent": lambda r: r.get("type") == "assistant" and bool(blocks(r, "tool_use", "Agent")),
        "assistant_sendmessage": lambda r: r.get("type") == "assistant" and bool(blocks(r, "tool_use", "SendMessage")),
        "assistant_workflow": lambda r: r.get("type") == "assistant" and bool(blocks(r, "tool_use", "Workflow")),
        "assistant_askuserquestion": lambda r: r.get("type") == "assistant" and bool(blocks(r, "tool_use", "AskUserQuestion")),
        "assistant_ask_operator": lambda r: r.get("type") == "assistant" and any(b.get("name", "").endswith("ask_operator") for b in blocks(r, "tool_use")),
        "assistant_bash": lambda r: r.get("type") == "assistant" and bool(blocks(r, "tool_use", "Bash")),
        "result_agent_async": lambda r: r.get("type") == "user" and tur(r, "status", "async_launched") and tur(r, "agentId"),
        "result_agent_foreground": lambda r: r.get("type") == "user" and tur(r, "status", "completed") and tur(r, "agentId"),
        "result_sendmessage": lambda r: r.get("type") == "user" and tur(r, "message") and "queued for delivery" in str(r["toolUseResult"].get("message")),
        "result_workflow": lambda r: r.get("type") == "user" and tur(r, "runId"),
        "result_ask_operator": lambda r: r.get("type") == "user" and any("Recorded as Q" in json.dumps(b.get("content")) for b in blocks(r, "tool_result")),
        "result_bash": lambda r: r.get("type") == "user" and tur(r, "stdout"),
        "result_sendmessage_resume": lambda r: r.get("type") == "user" and tur(r, "resumedAgentId"),
        "user_prompt_human": lambda r: r.get("type") == "user" and isinstance((r.get("message") or {}).get("content"), str)
        and (r.get("origin") or {}).get("kind") == "human",
        "custom_title": lambda r: r.get("type") == "custom-title",
        "agent_name": lambda r: r.get("type") == "agent-name",
        "ai_title": lambda r: r.get("type") == "ai-title",
        "queue_enqueue_notification": lambda r: r.get("type") == "queue-operation" and r.get("operation") == "enqueue"
        and str(r.get("content", "")).startswith("<task-notification>"),
        "attachment_queued_command": lambda r: r.get("type") == "attachment" and (r.get("attachment") or {}).get("type") == "queued_command"
        and str(r["attachment"].get("prompt", "")).startswith("<task-notification>"),
        # background work (design turn 8)
        "assistant_bash_background": lambda r: r.get("type") == "assistant" and any(
            (b.get("input") or {}).get("run_in_background") is True for b in blocks(r, "tool_use", "Bash")),
        "result_bash_background": lambda r: r.get("type") == "user" and tur(r, "backgroundTaskId") and not tur(r, "timedOutAfterMs"),
        "result_bash_timed_out_background": lambda r: r.get("type") == "user" and tur(r, "backgroundTaskId") and tur(r, "timedOutAfterMs"),
        "assistant_monitor": lambda r: r.get("type") == "assistant" and bool(blocks(r, "tool_use", "Monitor")),
        "result_monitor": lambda r: r.get("type") == "user" and tur(r, "taskId") and tur(r, "timeoutMs"),
        "assistant_taskstop": lambda r: r.get("type") == "assistant" and bool(blocks(r, "tool_use", "TaskStop")),
        "result_taskstop": lambda r: r.get("type") == "user" and tur(r, "task_type") and tur(r, "task_id"),
        "queue_enqueue_orphan_notice": lambda r: r.get("type") == "queue-operation" and r.get("operation") == "enqueue"
        and "__orphan_summary__" in str(r.get("content", "")),
        "queue_enqueue_monitor_event": lambda r: r.get("type") == "queue-operation" and r.get("operation") == "enqueue"
        and "<event>" in str(r.get("content", "")) and "<status>" not in str(r.get("content", ""))
        and "[Monitor expired" not in str(r.get("content", "")),
        "queue_enqueue_monitor_expired": lambda r: r.get("type") == "queue-operation" and r.get("operation") == "enqueue"
        and "[Monitor expired" in str(r.get("content", "")),
    }
    out: dict = {}
    for kind, pred in preds.items():
        for path in mains:
            rec = first(path, pred)
            if rec:
                out[kind] = anon(rec)
                break
    # the continuation rule (D6): continued-in and the bookkeeping records Claude Code writes after
    # it; rare, so every main transcript is searched (newest first), not only the largest
    book = {
        "continued_in": lambda r: r.get("type") == "continued-in",
        "last_prompt": lambda r: r.get("type") == "last-prompt",
        "cost_state": lambda r: r.get("type") == "cost-state",
        "mode": lambda r: r.get("type") == "mode",
        # undated at the top, dated inside (snapshot.timestamp): an undated continued-in's time
        "file_history_snapshot": lambda r: r.get("type") == "file-history-snapshot" and "timestamp" not in r
        and isinstance((r.get("snapshot") or {}).get("timestamp"), str),
    }
    every = sorted(glob.glob(os.path.join(root, "*", "*.jsonl")), key=os.path.getmtime, reverse=True)
    for kind, pred in book.items():
        for path in every:
            rec = first(path, pred)
            if rec:
                out[kind] = anon(rec)
                break
    sub_preds = {
        "sub_first_user": lambda r: r.get("type") == "user" and isinstance((r.get("message") or {}).get("content"), str) and r.get("agentId"),
        "sub_assistant_bash": lambda r: r.get("type") == "assistant" and bool(blocks(r, "tool_use", "Bash")) and r.get("agentId"),
        "sub_result_bash": lambda r: r.get("type") == "user" and tur(r, "stdout") and r.get("agentId"),
        "sub_assistant_text": lambda r: r.get("type") == "assistant" and bool(blocks(r, "text")) and r.get("agentId"),
        "sub_assistant_handback": lambda r: r.get("type") == "assistant" and bool(blocks(r, "tool_use", "SubagentHandback")),
        "sub_assistant_agent": lambda r: r.get("type") == "assistant" and bool(blocks(r, "tool_use", "Agent")) and r.get("agentId"),
        "sub_result_agent_async": lambda r: r.get("type") == "user" and tur(r, "status", "async_launched") and r.get("agentId"),
        "sub_attachment_queued_command": lambda r: r.get("type") == "attachment" and (r.get("attachment") or {}).get("type") == "queued_command"
        and str(r["attachment"].get("prompt", "")).startswith("<task-notification>") and r.get("agentId"),
        "sub_result_bash_background": lambda r: r.get("type") == "user" and tur(r, "backgroundTaskId") and r.get("agentId"),
    }
    for kind, pred in sub_preds.items():
        for path in subs:
            rec = first(path, pred)
            if rec:
                out[kind] = anon(rec)
                break
    metas = sorted(glob.glob(os.path.join(root, "*", "*", "subagents", "**", "agent-*.meta.json"), recursive=True))
    want = {"meta_background": lambda m: m.get("requestShape") == "background" and m.get("spawnDepth") == 1 and "parentAgentId" not in m,
            "meta_nested": lambda m: "parentAgentId" in m,
            "meta_workflow": lambda m: "workflowPhase" in m,
            "meta_fork": lambda m: m.get("agentType") == "fork"}
    for kind, pred in want.items():
        for p in metas:
            try:
                m = json.load(open(p))
            except (OSError, ValueError):
                continue
            if pred(m):
                out[kind] = anon(m)
                break
    for p in wf_journals:
        lines = [json.loads(x) for x in open(p) if x.strip()]
        for kind in ("launched", "started", "result", "failed"):
            rec = next((x for x in lines if x.get("type") == kind), None)
            if rec:
                out[f"journal_{kind}"] = anon(rec)
        if all(f"journal_{k}" in out for k in ("launched", "started", "result", "failed")):
            break
    if wf_summaries:
        data = json.load(open(wf_summaries[0]))
        data["phases"] = data.get("phases", [])[:1]
        data["workflowProgress"] = [p for p in data.get("workflowProgress", []) if p.get("type") == "workflow_phase"][:1] + \
                                   [p for p in data.get("workflowProgress", []) if p.get("type") == "workflow_agent"][:1]
        data["script"] = ""
        out["workflow_summary"] = anon(data)
    if merge:
        old = json.load(open(merge))
        out = {**old, **{k: v for k, v in out.items() if k not in old}}
    print(json.dumps(out, indent=1))  # real key order is part of the wire format




def capture_snapshot() -> None:
    """``herdr api snapshot | capture.py --snapshot``: one anonymised entry per list."""
    data = json.load(sys.stdin)
    snap = data["result"]["snapshot"]
    keep = {"agent", "agent_status", "focused", "zoomed", "protocol", "version", "type"}
    def scrub(v, k=None):
        if isinstance(v, dict):
            return {kk: scrub(vv, kk) for kk, vv in v.items()}
        if isinstance(v, list):
            return [scrub(v[0], k)] if v else []
        if isinstance(v, (bool, int, float)) or v is None or k in keep:
            return v
        return ""
    out = {"id": data.get("id"), "result": {"snapshot": scrub(snap), "type": data["result"].get("type")}}
    if merge:
        old = json.load(open(merge))
        out = {**old, **{k: v for k, v in out.items() if k not in old}}
    print(json.dumps(out, indent=1))  # real key order is part of the wire format


if __name__ == "__main__" and "--snapshot" in sys.argv:
    capture_snapshot()


if __name__ == "__main__" and "--snapshot" not in sys.argv:
    main()
