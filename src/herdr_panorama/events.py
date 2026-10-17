"""The explicit thread channel (v1): an append-only JSONL file per session.

``${PANORAMA_EVENTS_DIR:-${XDG_STATE_HOME:-~/.local/state}/herdr-panorama/events}/<session_id>.jsonl``

Each line declares or updates one thread; later lines update earlier ones
field by field (``phases`` and ``agents`` replace). See docs/DESIGN.md.
"""

from __future__ import annotations

import json
import math
import os
import time
from dataclasses import dataclass, field
from pathlib import Path

from .scan import discovery, tail, wire

VERSION = 1
MAX_ITEMS = 30  # phases and agents per thread
MAX_TEXT = 200
MIN_PREFIX = 4  # an agent pattern "abc*" must name at least this much before the *
COMPACT_AT = 64 * 1024  # rewrite the events file as one line per thread past this size
STATES = {"done", "now", "next", "failed", "skipped"}
STATUSES = {"active", "done", "failed", "cancelled"}


def events_dir() -> Path:
    env = os.environ.get("PANORAMA_EVENTS_DIR")
    if env:
        return Path(env)
    state = os.environ.get("XDG_STATE_HOME") or os.path.join(os.path.expanduser("~"), ".local", "state")
    return Path(state) / "herdr-panorama" / "events"


def events_path(session_id: str) -> Path:
    safe = discovery.canonical_session_id(session_id) or "".join(c for c in session_id if c.isalnum() or c in "-_")
    return events_dir() / f"{safe}.jsonl"


@dataclass
class Declared:
    thread: str
    name: str | None = None
    flavour: str | None = None
    status: str | None = None
    worktree: str | None = None
    pct: int | None = None
    phases: list[dict] = field(default_factory=list)
    agents: list[dict] = field(default_factory=list)
    ts: float | None = None
    order: int = 0


class Channel:
    """Follows one session's events file; ``threads`` holds the merged declarations."""

    def __init__(self, session_id: str):
        self.path = str(events_path(session_id))
        self.state = tail.TailState()
        self.threads: dict[str, Declared] = {}

    def update(self) -> None:
        kind, lines = tail.read_appended(self.path, self.state)
        if kind == tail.RESET:
            self.threads.clear()
            kind, lines = tail.read_appended(self.path, self.state)
        for raw in lines:
            rec = wire.parse_line(raw)
            if rec is not None:
                self.apply(rec)

    def apply(self, rec: dict) -> None:
        if rec.get("v") != VERSION or not isinstance(rec.get("thread"), str) or not rec["thread"]:
            return
        tid = rec["thread"]
        d = self.threads.get(tid)
        if d is None:
            d = self.threads[tid] = Declared(tid, order=len(self.threads))
        for key in ("name", "flavour", "worktree"):
            if isinstance(rec.get(key), str):
                setattr(d, key, rec[key])
        if rec.get("status") in STATUSES:
            d.status = rec["status"]
        pct = rec.get("pct")
        if isinstance(pct, (int, float)) and not isinstance(pct, bool) and math.isfinite(pct):
            d.pct = max(0, min(100, int(pct)))
        if isinstance(rec.get("phases"), list):
            d.phases = [p for p in rec["phases"] if isinstance(p, dict) and isinstance(p.get("name"), str)]
        if isinstance(rec.get("agents"), list):
            d.agents = [a for a in rec["agents"] if isinstance(a, dict) and isinstance(a.get("match"), str)]
        d.ts = wire.ts(rec.get("ts")) or d.ts


def matches(pattern: str, agent_id: str, description: str) -> bool:
    if pattern == agent_id:
        return True
    if pattern.endswith("*"):
        prefix = pattern.rstrip("*")
        return len(prefix) >= MIN_PREFIX and description.startswith(prefix)  # "*" never claims everything
    return description == pattern


def phase_time(value, ref: float | None) -> float | None:
    """``HH:MM`` (local, on the reference day) or ISO-8601 -> POSIX seconds."""
    if not isinstance(value, str) or not value:
        return None
    if len(value) <= 5 and ":" in value:
        try:
            hh, mm = (int(x) for x in value.split(":"))
        except ValueError:
            return None
        base = time.localtime(ref or time.time())
        return time.mktime((base.tm_year, base.tm_mon, base.tm_mday, hh, mm, 0, 0, 0, -1))
    return wire.ts(value)


def _locked(path: Path):
    """An exclusive lock beside the events file, so a compaction never drops an append."""
    import fcntl
    path.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(str(path) + ".lock", os.O_WRONLY | os.O_CREAT, 0o644)
    fcntl.flock(fd, fcntl.LOCK_EX)
    return fd


def _unlock(fd: int) -> None:
    import fcntl
    fcntl.flock(fd, fcntl.LOCK_UN)
    os.close(fd)


def emit(session_id: str, record: dict) -> Path:
    """Append one declaration line (one write call, O_APPEND); compact the file when it grew."""
    record = {"v": VERSION, "ts": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), **record}
    path = events_path(session_id)
    data = (json.dumps(record, ensure_ascii=False) + "\n").encode()
    lock = _locked(path)
    try:
        fd = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o644)
        try:
            os.write(fd, data)
        finally:
            os.close(fd)
        if path.stat().st_size > COMPACT_AT:
            _compact(session_id, path)
    finally:
        _unlock(lock)
    return path


def _compact(session_id: str, path: Path) -> None:
    """Rewrite the file as one full declaration per thread (a new inode: readers re-read it)."""
    ch = Channel(session_id)
    ch.update()
    tmp = path.with_suffix(".jsonl.tmp")
    with open(tmp, "w", encoding="utf-8") as fh:
        for d in sorted(ch.threads.values(), key=lambda d: d.order):
            rec = {"v": VERSION, "thread": d.thread, "phases": d.phases, "agents": d.agents}
            if d.ts:
                rec["ts"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(d.ts))
            for key in ("name", "flavour", "status", "worktree", "pct"):
                if getattr(d, key) is not None:
                    rec[key] = getattr(d, key)
            fh.write(json.dumps(rec, ensure_ascii=False) + "\n")
    os.replace(tmp, path)


def current(session_id: str, thread: str) -> Declared | None:
    ch = Channel(session_id)
    ch.update()
    return ch.threads.get(thread)


def report(session_id: str, thread: str, title: str | None = None, phase: str | None = None,
           phases_done: list[str] | None = None, next_phases: list[str] | None = None,
           status: str | None = None, agents: list[str] | None = None, now: float | None = None) -> dict:
    """Merge an agent's report into the thread's declaration and append it.

    Phases keep their start/end times across calls: a phase gets ``start`` when it is first
    reported as current, and ``end`` when it is first reported as done. Omitted lists keep
    what was declared before; ``phase`` moves to the front of the not-done phases.
    """
    now = time.time() if now is None else now
    stamp = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(now))
    prev = current(session_id, thread)
    old = {p["name"]: p for p in (prev.phases if prev else [])}
    done_names = list(phases_done) if phases_done is not None else [n for n, p in old.items() if p.get("state") == "done"]
    if phase:
        done_names = [n for n in done_names if n != phase]
    if next_phases is not None:
        next_names = [n for n in next_phases if n != phase and n not in done_names]
    else:
        next_names = [n for n, p in old.items() if p.get("state") == "next" and n != phase and n not in done_names]
    phases = []
    for n in done_names:
        p = dict(old.get(n, {"name": n}))
        p["state"] = "done"
        p.setdefault("start", p.get("end") or stamp)
        if old.get(n, {}).get("state") != "done" or "end" not in p:
            p["end"] = stamp
        phases.append(p)
    if phase:
        p = dict(old.get(phase, {"name": phase}))
        p["state"] = "failed" if status == "failed" else ("done" if status == "done" else "now")
        p.setdefault("start", stamp)
        if p["state"] == "done":
            p["end"] = stamp
        phases.append(p)
    for n in next_names:
        phases.append({"name": n, "state": "next"})
    rec: dict = {"thread": thread, "phases": phases}
    if title:
        rec["name"] = title
    if status in STATUSES:
        rec["status"] = status
    elif prev is None or prev.status is None:
        rec["status"] = "active"
    if agents:
        rec["agents"] = [{"match": a} for a in agents]
    emit(session_id, rec)
    return {"thread": thread, "phases": [(p["name"], p["state"]) for p in phases], "thread_status": rec.get("status")}
