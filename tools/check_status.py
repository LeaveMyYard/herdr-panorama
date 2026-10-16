"""Rule-level cross-check of subagent statuses at one moment (read-only).

Not an oracle: it applies the same rules as the scanner, so it only catches
implementation slips. The accuracy gate is tools/replay_oracle.py, which scores
the scanner against what agents did afterwards.

Deliberately shares no code with herdr_panorama: it reads every line of every
file of a session with plain JSON parsing and applies the reference rules the
zoetrope study used (docs/DESIGN.md "Accuracy"):

1. a workflow journal ``result`` for the agent -> done;
2. otherwise the latest ``<task-notification>`` for the agent, from any
   carrier (user string, queue-operation enqueue, attachment queued_command)
   in any file of the session (completed -> done, failed/error -> failed,
   killed/stopped -> stopped), unless a ``SendMessage`` resume
   (``resumedAgentId``) or the agent's own activity comes more than 5 s later;
   a foreground ``Agent`` result with status ``completed`` counts as such a
   notice; a workflow agent without a result whose run's latest launch got a
   terminal notice -> stopped;
3. otherwise running if the agent wrote within 120 s, or has a tool call
   without result that is at most 1 h old; else done (ended silently).

Usage: python tools/check_status.py <session.jsonl> [--now EPOCH]  -> JSON {agent_id: status}
"""

from __future__ import annotations

import glob
import json
import os
import re
import sys
import time
from datetime import datetime

TAG = re.compile(r"<(task-id|status)>\s*([^<]*?)\s*</\1>")
BLOCK = re.compile(r"<task-notification>.*?</task-notification>", re.S)
STATUS = {"completed": "done", "failed": "failed", "error": "failed", "killed": "stopped", "stopped": "stopped"}


def when(v):
    try:
        return datetime.fromisoformat(str(v).replace("Z", "+00:00")).timestamp()
    except (ValueError, TypeError):
        return None


def records(path):
    try:
        fh = open(path, encoding="utf-8", errors="replace")
    except OSError:
        return
    with fh:
        for line in fh:
            try:
                r = json.loads(line)
            except ValueError:
                continue
            if isinstance(r, dict):
                yield r


def notice_of(r):
    t = r.get("type")
    text = None
    if t == "queue-operation" and r.get("operation") == "enqueue":
        text = r.get("content")
    elif t == "attachment" and isinstance(r.get("attachment"), dict) and r["attachment"].get("type") == "queued_command":
        text = r["attachment"].get("prompt")
    elif t == "user" and isinstance(r.get("message"), dict) and isinstance(r["message"].get("content"), str):
        text = r["message"]["content"]
    if not isinstance(text, str) or not text.lstrip().startswith("<task-notification>"):
        return []
    out = []
    for blk in BLOCK.findall(text):  # a carrier can hold several blocks
        tags = dict(TAG.findall(blk))
        st = STATUS.get(tags.get("status", ""))
        if tags.get("task-id") and st:
            out.append((tags["task-id"], st))
    return out


def check(root: str, now: float) -> dict[str, str]:
    base = root[: -len(".jsonl")]
    agent_files = {}
    for p in glob.glob(os.path.join(base, "subagents", "agent-*.jsonl")):
        agent_files[os.path.basename(p)[6:-6]] = (p, None)
    for p in glob.glob(os.path.join(base, "subagents", "workflows", "*", "agent-*.jsonl")):
        agent_files[os.path.basename(p)[6:-6]] = (p, os.path.basename(os.path.dirname(p)))
    notices, resumes, journal_done, journal_failed = {}, {}, set(), set()
    run_tasks = {}  # runId -> [(ts, taskId)]
    last, pending = {}, {}
    files = [(root, None)] + [(p, aid) for aid, (p, _) in agent_files.items()]
    for path, owner in files:
        uses, results = {}, set()
        for r in records(path):
            ts = when(r.get("timestamp"))
            if owner and ts is not None and r.get("type") in ("user", "assistant", "system"):
                last[owner] = max(last.get(owner, 0.0), ts)
            for n in notice_of(r):
                if ts is not None:
                    notices.setdefault(n[0], []).append((ts, n[1]))
            tur = r.get("toolUseResult")
            if isinstance(tur, dict) and ts is not None:
                if tur.get("resumedAgentId") and tur.get("success") is not False:
                    resumes.setdefault(tur["resumedAgentId"], []).append(ts)
                if tur.get("status") == "completed" and tur.get("agentId"):
                    notices.setdefault(tur["agentId"], []).append((ts, "done"))
                if tur.get("runId") and tur.get("taskId"):
                    run_tasks.setdefault(tur["runId"], []).append((ts, tur["taskId"]))
            content = r.get("message", {}).get("content") if isinstance(r.get("message"), dict) else None
            if isinstance(content, list):
                for b in content:
                    if isinstance(b, dict) and b.get("type") == "tool_use" and r.get("type") == "assistant":
                        uses[b.get("id")] = ts
                    elif isinstance(b, dict) and b.get("type") == "tool_result":
                        results.add(b.get("tool_use_id"))
        if owner:
            pending[owner] = [t for k, t in uses.items() if k not in results and t is not None]
    for j in glob.glob(os.path.join(base, "subagents", "workflows", "*", "journal.jsonl")):
        for r in records(j):
            if r.get("type") == "result" and r.get("agentId"):
                journal_done.add(r["agentId"])
            elif r.get("type") == "failed" and r.get("agentId"):
                journal_failed.add(r["agentId"])
    out = {}
    for aid, (_, run) in agent_files.items():
        if aid in journal_failed:
            out[aid] = "failed"
            continue
        if aid in journal_done:
            out[aid] = "done"
            continue
        ev = list(notices.get(aid, []))
        if run and run in run_tasks:
            task = max(run_tasks[run])[1]
            if notices.get(task):
                t, st = max(notices[task])
                ev.append((t, "stopped" if st in ("stopped", "failed") else st))
        lt = last.get(aid)
        if ev:
            t, st = max(ev)
            if not any(r > t for r in resumes.get(aid, [])) and not (lt is not None and lt > t + 5):
                out[aid] = st
                continue
        if lt is not None and (now - lt <= 120 or any(now - p <= 3600 for p in pending.get(aid, []))):
            out[aid] = "running"
        else:
            out[aid] = "done"
    return out


if __name__ == "__main__":
    now = float(sys.argv[sys.argv.index("--now") + 1]) if "--now" in sys.argv else time.time()
    print(json.dumps(check(sys.argv[1], now), indent=1, sort_keys=True))
