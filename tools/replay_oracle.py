"""Hindsight replay oracle (accuracy gate, review M4). Read-only; prints counts only.

For each session, every file is truncated at past instants T (K spread over
the session, K inside activity gaps: a random agent write plus 2-30 min) (copies under
``.scratch/replay``), the scanner's production path runs on the copy with
``now = T`` (``Session.update(since=T - window - 600)``), and its answer is
scored against what the agent actually did afterwards, from the full files:

- truth "running at T": the agent wrote again within 30 min after T, or a
  notice for it arrived within 30 min after T, and no terminal evidence (a notice, a completed
  acknowledgement, a workflow journal result/failed) lies between its last
  write at or before T and T;
- truth parent: the owner of the ``tool_use`` whose id is the agent's
  ``meta.toolUseId`` (main, or the agent file that contains it first).

This oracle does not know the scanner's rules (windows, carriers, resumes): it
only knows the future.

    uv run python tools/replay_oracle.py [--src SRC] [--instants 24] [--newest 8] [prefix ...]
"""

from __future__ import annotations

import glob
import importlib
import json
import os
import random
import re
import shutil
import sys
from datetime import datetime

PROJECTS = os.path.join(os.path.expanduser("~"), ".claude", "projects")
SCRATCH = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), ".scratch", "replay")
BLOCK = re.compile(r"<task-notification>.*?</task-notification>", re.S)
TAG = re.compile(r"<(task-id|status)>\s*([^<]*?)\s*</\1>")
RUNNING = {"run", "rev", "wait", "blk", "q", "bg"}  # bg: waiting on its own background work (turn 8)
WINDOW = 3600.0
HORIZON = 30 * 60.0  # "running at T" = shows life again within 30 min (the reviewer's oracle)


def ts_of(v):
    try:
        return datetime.fromisoformat(str(v).replace("Z", "+00:00")).timestamp()
    except (ValueError, TypeError):
        return None


def lines_with_ts(path):
    """[(end_offset, ts or inherited, record or None)]"""
    out, last, off = [], None, 0
    with open(path, "rb") as fh:
        for raw in fh:
            off += len(raw)
            if not raw.endswith(b"\n"):
                break
            try:
                r = json.loads(raw)
            except ValueError:
                r = None
            t = ts_of(r.get("timestamp")) if isinstance(r, dict) else None
            last = t if t is not None else last
            out.append((off, last, r if isinstance(r, dict) else None))
    return out


def carrier_text(r):
    t = r.get("type")
    if t == "queue-operation" and r.get("operation") == "enqueue":
        return r.get("content")
    if t == "attachment" and isinstance(r.get("attachment"), dict):
        return r["attachment"].get("prompt")
    if t == "user" and isinstance(r.get("message"), dict) and isinstance(r["message"].get("content"), str):
        return r["message"]["content"]
    return None


class Truth:
    def __init__(self, root):
        self.root = root
        base = root[:-6]
        self.agent_files = {}
        for p in glob.glob(os.path.join(base, "subagents", "agent-*.jsonl")):
            self.agent_files[os.path.basename(p)[6:-6]] = (p, None)
        for p in glob.glob(os.path.join(base, "subagents", "workflows", "*", "agent-*.jsonl")):
            self.agent_files[os.path.basename(p)[6:-6]] = (p, os.path.basename(os.path.dirname(p)))
        self.lines = {root: lines_with_ts(root)}
        for p, _ in self.agent_files.values():
            self.lines[p] = lines_with_ts(p)
        self.writes = {a: sorted(t for _, t, r in self.lines[p] if r is not None and t is not None
                                 and r.get("type") in ("user", "assistant", "system"))
                       for a, (p, _) in self.agent_files.items()}
        self.notices, self.terminal, self.tool_owner = {}, {}, {}
        for path, ls in self.lines.items():
            owner = "main" if path == root else os.path.basename(path)[6:-6]
            for _, t, r in ls:
                if r is None:
                    continue
                text = carrier_text(r)
                if isinstance(text, str) and text.lstrip().startswith("<task-notification>") and t is not None:
                    for blk in BLOCK.findall(text):
                        tags = dict(TAG.findall(blk))
                        if tags.get("task-id"):
                            self.notices.setdefault(tags["task-id"], []).append(t)
                            self.terminal.setdefault(tags["task-id"], []).append(t)
                tur = r.get("toolUseResult")
                if isinstance(tur, dict) and tur.get("status") == "completed" and tur.get("agentId") and t is not None:
                    self.terminal.setdefault(tur["agentId"], []).append(t)
                content = r.get("message", {}).get("content") if isinstance(r.get("message"), dict) else None
                if r.get("type") == "assistant" and isinstance(content, list):
                    for b in content:
                        if isinstance(b, dict) and b.get("type") == "tool_use" and b.get("id"):
                            self.tool_owner.setdefault(b["id"], []).append((owner, t or 0))
        for j in glob.glob(os.path.join(base, "subagents", "workflows", "*", "journal.jsonl")):
            for raw in open(j, "rb"):
                try:
                    r = json.loads(raw)
                except ValueError:
                    continue
                a = r.get("agentId")
                if r.get("type") in ("result", "failed") and a in self.writes and self.writes[a]:
                    self.terminal.setdefault(a, []).append(self.writes[a][-1])
        self.meta = {}
        for a, (p, _) in self.agent_files.items():
            try:
                self.meta[a] = json.load(open(p[:-6] + ".meta.json"))
            except (OSError, ValueError):
                self.meta[a] = {}

    def span(self):
        ts = [t for ls in self.lines.values() for _, t, _ in ls if t is not None]
        return (min(ts), max(ts)) if ts else (None, None)

    def exists(self, a, T):
        w = self.writes.get(a) or []
        return bool(w) and w[0] <= T

    def running(self, a, T):
        w = self.writes.get(a) or []
        before = [x for x in w if x <= T]
        later = any(T < x <= T + HORIZON for x in w) or any(T < x <= T + HORIZON for x in self.notices.get(a, []))
        if not later or not before:
            return False
        return not any(before[-1] <= x <= T for x in self.terminal.get(a, []))

    def parent(self, a):
        """Who made the call: main if it is in the main transcript, else the earliest other file
        (an agent's own file can repeat the call that spawned it: forks copy history)."""
        owners = [o for o in self.tool_owner.get(self.meta.get(a, {}).get("toolUseId"), []) if o[0] != a]
        if not owners:
            return None
        if any(o[0] == "main" for o in owners):
            return "main"
        return min(owners, key=lambda o: o[1])[0]

    def truncate(self, T, dest):
        shutil.rmtree(dest, ignore_errors=True)
        base_src = self.root[:-6]
        rel = lambda p: os.path.relpath(p, os.path.dirname(self.root))  # noqa: E731
        for path, ls in self.lines.items():
            cut = 0
            for off, t, _ in ls:
                if t is not None and t > T:
                    break
                cut = off
            out = os.path.join(dest, rel(path))
            os.makedirs(os.path.dirname(out), exist_ok=True)
            with open(path, "rb") as src, open(out, "wb") as dst:
                dst.write(src.read(cut))
            if path != self.root and cut > 0:
                meta = path[:-6] + ".meta.json"
                if os.path.exists(meta):
                    shutil.copy(meta, out[:-6] + ".meta.json")
        for j in glob.glob(os.path.join(base_src, "subagents", "workflows", "*", "journal.jsonl")):
            keep = []
            for raw in open(j, "rb"):
                try:
                    r = json.loads(raw)
                except ValueError:
                    continue
                a = r.get("agentId")
                w = self.writes.get(a) or []
                if r.get("type") == "launched" or (r.get("type") == "started" and w and w[0] <= T) or (
                        r.get("type") in ("result", "failed") and w and w[-1] <= T):
                    keep.append(raw)
            out = os.path.join(dest, rel(j))
            os.makedirs(os.path.dirname(out), exist_ok=True)
            open(out, "wb").write(b"".join(keep))
        for s in glob.glob(os.path.join(base_src, "workflows", "*.json")):
            if os.path.getmtime(s) <= T:
                out = os.path.join(dest, rel(s))
                os.makedirs(os.path.dirname(out), exist_ok=True)
                shutil.copy(s, out)


def main() -> None:
    args = sys.argv[1:]
    src = args[args.index("--src") + 1] if "--src" in args else os.path.join(os.path.dirname(SCRATCH), "..", "src")
    k = int(args[args.index("--instants") + 1]) if "--instants" in args else 24
    newest = int(args[args.index("--newest") + 1]) if "--newest" in args else 8
    flags = {"--src", "--instants", "--newest"}
    prefixes = [a for i, a in enumerate(args) if not a.startswith("--") and (i == 0 or args[i - 1] not in flags)]
    sys.path.insert(0, os.path.abspath(src))
    session_mod = importlib.import_module("herdr_panorama.scan.session")
    details = "--details" in args
    if details:
        print("scanner:", session_mod.__file__)
    roots = []
    for p in prefixes:
        roots += glob.glob(os.path.join(PROJECTS, "*", f"{p}*.jsonl"))
    with_subs = [r for r in glob.glob(os.path.join(PROJECTS, "*", "*.jsonl")) if os.path.isdir(r[:-6] + "/subagents")]
    roots += [r for r in sorted(with_subs, key=os.path.getmtime, reverse=True)[:newest] if r not in roots]
    total = {"agent_instants": 0, "truth_running": 0, "false_done": 0, "false_running": 0, "parent_checked": 0,
             "parent_wrong": 0, "bg": 0, "bg_woken_30m": 0, "bg_woken_later": 0}
    for root in roots:
        truth = Truth(root)
        lo, hi = truth.span()
        if lo is None or hi - lo < 60:
            continue
        c = dict.fromkeys(total, 0)
        # k instants spread over the session, plus k inside activity gaps: a random agent
        # write + 2..30 min (where silence-based rules are tested); seeded, reproducible
        rng = random.Random(root)
        writes = [x for w in truth.writes.values() for x in w]
        gaps = [rng.choice(writes) + rng.uniform(130, 1800) for _ in range(k)] if writes else []
        instants = [lo + (hi - lo) * i / (k + 1) for i in range(1, k + 1)] + sorted(gaps)
        for T in [t for t in instants if t <= hi - HORIZON]:  # the last 30 min have no future to score
            dest = os.path.join(SCRATCH, "copy")
            truth.truncate(T, dest)
            copy_root = os.path.join(dest, os.path.basename(root))
            s = session_mod.Session(os.path.basename(root)[:-6], copy_root)
            try:
                s.update(since=T - WINDOW - 600)
            except TypeError:  # an older Session.update without `since`
                s.update()
            for a in truth.agent_files:
                if not truth.exists(a, T):
                    continue
                if a not in s.agents or s.file_of(a) is None or s.file_of(a).tail.offset == 0:
                    if not truth.running(a, T):
                        continue  # skipped as outside the window and not running: nothing shown, nothing wrong
                status = s.status_of(a, T)[0]
                c["agent_instants"] += 1
                want = truth.running(a, T)
                got = status in RUNNING
                c["truth_running"] += want
                c["false_done"] += want and not got
                if details and want != got:
                    f = s.file_of(a)
                    w = truth.writes.get(a) or []
                    nxt = min([x for x in w if x > T] or [float("inf")])
                    prev = max([x for x in w if x <= T] or [0])
                    sp = s.spawn_of(a)
                    print(f"   {a} T+{T - lo:.0f}s got={status} since_last={T - prev:.0f}s next_write_in={nxt - T:.0f}s "
                          f"pending={len(f.info.pending) if f else '-'} fg={sp is not None and not sp.background} "
                          f"spawn_result={sp is not None and sp.call in getattr(s, 'spawn_results', set())} "
                          f"shape={truth.meta.get(a, {}).get('requestShape')} wf={truth.agent_files[a][1] is not None}")
                c["false_running"] += got and not want
                if status == "bg":  # waiting: was it woken again (a write or a notice) at all, later?
                    c["bg"] += 1
                    c["bg_woken_30m"] += want
                    c["bg_woken_later"] += any(x > T for x in truth.writes.get(a, []) + truth.notices.get(a, []))
                    if "--bg-details" in args:
                        nxt = min([x for x in truth.writes.get(a, []) + truth.notices.get(a, []) if x > T] or [float("inf")])
                        tasks = [(x.task, x.kind, (round(min(truth.notices[x.task]) - T) if truth.notices.get(x.task) else None))
                                 for x in s.waiting_bg(a, T)]
                        term = [round(x - T) for x in truth.terminal.get(a, []) if x > T][:1]
                        print(f"   bg {a[:10]} T+{T - lo:.0f}s woken_in={nxt - T:.0f}s agent_end_in={term} "
                              f"tasks(id,kind,notice_in_s)={tasks}")
                p = truth.parent(a)
                if p is not None and truth.meta.get(a, {}).get("toolUseId"):
                    c["parent_checked"] += 1
                    wrong = (s.parent_of(a) or "main") != p
                    c["parent_wrong"] += wrong
                    if details and wrong:
                        print(f"   parent {a} got={s.parent_of(a)} want={p} depth={truth.meta.get(a, {}).get('spawnDepth')} "
                              f"type={truth.meta.get(a, {}).get('agentType')}")
        shutil.rmtree(os.path.join(SCRATCH, "copy"), ignore_errors=True)
        print(os.path.basename(root)[:8], json.dumps(c))
        for key in total:
            total[key] += c[key]
    print("TOTAL", json.dumps(total))


if __name__ == "__main__":
    main()
