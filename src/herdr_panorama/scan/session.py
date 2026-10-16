"""One Claude Code session: its files tailed incrementally and folded into keyed stores.

Structure after zoetrope ``src/state/session.rs`` (MIT): records are folded into
stores keyed by what they are about (spawn calls, acknowledgements, notices,
resumes, journal results), so the result does not depend on the order files or
lines are read; statuses are derived at query time from the stores and the
clock (``status_of``).

Status rules (study §3 and §6, fixing zoetrope's three errors on our data):

1. Terminal evidence is, newest first: the latest ``<task-notification>`` for
   the agent from any carrier in any file (``completed`` -> done, ``failed`` ->
   fail, ``killed``/``stopped`` -> cxl), a foreground spawn's ``completed``
   acknowledgement, a workflow journal ``result`` (done), the end of the
   agent's killed workflow run (cxl).
2. Evidence is overturned by a later ``SendMessage`` resume
   (``resumedAgentId``) or by the agent's own activity more than
   ``REOPEN_MARGIN`` after it: the agent is running again.
3. Without standing terminal evidence an agent is running while active:
   written within ``ACTIVE`` seconds, or a tool call in flight for at most
   ``PENDING_CAP`` (an abandoned call no longer keeps it running forever), or
   a background agent that has not been notified yet, within ``ASYNC_QUIET``.
   Otherwise, if background work it started is still in flight (``waiting_bg``),
   it is waiting (``bg``, design turn 8); else it ended silently (done,
   ``inferred``).

Background work (``bg``): a Bash result with ``backgroundTaskId`` (run in the
background, or moved there by its timeout) or a Monitor result (``taskId`` with
``timeoutMs``) starts a task owned by the file's owner. It ends with any
terminal notice naming it (every ``<task-id>`` of a notice counts), a Monitor's
"[Monitor expired" event or its timeout, or a successful TaskStop / KillShell.
Claude Code can also end one without a record (a stop from its UI, the process
exiting), so an open task only counts while ``bg_alive`` says its process lives
(local machines: collect.py checks the Claude process's descendants), or, when
that cannot be checked, for at most ``BG_CAP`` after it started.

Parentage (study §1): ``meta.toolUseId`` is the ``tool_use.id`` of the spawning
call; the owner of that call (main or another agent's file) is the parent, so
depth-2 agents nest under the agent that spawned them.

Performance: a full-file pass only JSON-parses lines that contain a marker
(``wire.MARKERS``); each agent's current activity, pending call, last text
and model come from the last 64 KB, re-read only when the file grew; tool
calls are a byte count of ``"type":"tool_use"``.
"""

from __future__ import annotations

import json
import os
from dataclasses import dataclass, field

from .. import hooks as hookmod
from . import discovery, tail, wire

MAIN = "main"
ACTIVE = 120.0  # zoetrope INTERACTIVE_IDLE_SECS
PENDING_CAP = 3600.0
ASYNC_QUIET = 600.0
REOPEN_MARGIN = 5.0  # an agent's final line can land a moment after its own notice
TAIL_BYTES = 65536
TOOL_USE = b'"type":"tool_use"'
ASSISTANT = b'"type":"assistant"'
TAIL_MAX = 8 * 1024 * 1024  # the tail window grows up to this when the last records are huge
BG_MARK = b'"backgroundTaskId"'  # a Bash moved to the background (the result record)
BG_CAP = 24 * 3600.0  # an open background task nobody can check (remote, no /proc) counts this long
MONITOR_SLACK = 60.0  # a Monitor past its timeout by this much has expired even without its last event


@dataclass
class Spawn:
    call: str
    owner: str  # MAIN or the spawning agent's id
    ts: float | None
    description: str = ""
    model: str | None = None
    subagent_type: str | None = None
    background: bool = False


@dataclass
class Ack:
    call: str
    ts: float | None
    agent_id: str
    status: str  # async_launched | completed | ...
    is_error: bool = False
    model: str | None = None
    tool_count: int | None = None


@dataclass
class WorkflowLaunch:
    call: str
    ts: float | None
    run_id: str
    task_id: str | None
    name: str | None
    summary: str | None


@dataclass
class BgLaunch:
    task: str
    owner: str  # MAIN or the agent whose file launched it
    ts: float | None
    kind: str  # Bash | Monitor
    call: str
    timeout: float | None = None  # Monitor: seconds until it expires (unless persistent)
    persistent: bool = False


@dataclass
class TailInfo:
    first_ts: float | None = None
    last_ts: float | None = None
    model: str | None = None
    last_text: str = ""
    activity: str = ""
    pending: list[tuple[str, dict, float | None]] = field(default_factory=list)  # (name, input, ts)
    handback: bool = False


@dataclass
class FileState:
    path: str
    owner: str
    tail: tail.TailState = field(default_factory=tail.TailState)
    last_ts: float | None = None  # stream timestamp for undated records
    tool_calls: int = 0
    info: TailInfo = field(default_factory=TailInfo)
    info_size: int = -1


@dataclass
class AgentRec:
    id: str
    file: discovery.AgentFile | None = None
    meta: dict = field(default_factory=dict)
    meta_mtime: float = -1.0


class Session:
    def __init__(self, session_id: str, root: str, cwd: str | None = None):
        self.session_id, self.root, self.cwd = session_id, root, cwd
        self.files: dict[str, FileState] = {}
        self.agents: dict[str, AgentRec] = {}
        self.journals: dict[str, FileState] = {}
        self.hook_file = FileState(str(hookmod.hooks_path(session_id)), "hooks")
        self.bg_alive = lambda launch: None  # collect.py: True/False when the task's process can be checked
        self._bg_scanned: set[tuple[str, float]] = set()  # skipped old files searched for a launch
        self._reset_stores()

    def _reset_stores(self) -> None:
        self.spawns: dict[str, Spawn] = {}
        self.acks: dict[str, Ack] = {}
        self.ack_of: dict[str, str] = {}  # agentId -> call
        self.notices: dict[str, set[tuple[float, str, int | None]]] = {}
        self.resumes: dict[str, set[float]] = {}
        self.wf_launches: dict[str, WorkflowLaunch] = {}
        self.journal_started: dict[str, dict[str, tuple[str, str | None]]] = {}  # run -> agent -> (label, phase)
        self.journal_phases: dict[str, list[str]] = {}
        self.journal_done: dict[str, set[str]] = {}
        self.journal_failed: dict[str, set[str]] = {}
        self.spawn_results: set[str] = set()  # spawning calls that got a tool_result (any outcome)
        self.spawn_owners: dict[str, dict[str, float]] = {}  # call -> {file owner: earliest ts}
        self._children: dict[str, list[str]] | None = None
        self.wf_summary: dict[str, dict] = {}
        self.prompts: set[tuple[float, str]] = set()
        self.titles: dict[str, str] = {}
        self.continued_in: str | None = None  # the session this one continued in (its last word)
        self.qids: dict[str, set[int]] = {}
        self.bad_records = 0
        self._pending_ask: dict[str, str] = {}
        self._wf_mtime: dict[str, float] = {}
        self.hook_starts: dict[str, list[tuple[float, int | None, str | None, str | None]]] = {}  # hooks.py
        self.hook_stops: dict[str, list[float]] = {}
        self.hook_procs: list[tuple[float, int, str]] = []  # (ts, pid, start) of every hook record
        self._alive: dict[tuple[int, str], bool | None] = {}
        self.bg: dict[str, BgLaunch] = {}  # task id -> its launch
        self._bg_index = None
        self.bg_calls: dict[str, tuple[str, str]] = {}  # call -> (Bash | Monitor, description)
        self.bg_stop_calls: dict[str, str] = {}  # TaskStop / KillShell call -> task id
        self.bg_stops: dict[str, float] = {}  # task id -> when a TaskStop ended it
        if hasattr(self, "hook_file"):
            self.hook_file.tail = tail.TailState()
        for f in getattr(self, "files", {}).values():
            f.tail = tail.TailState()
            f.tool_calls, f.last_ts, f.info_size = 0, None, -1
        for j in getattr(self, "journals", {}).values():
            j.tail = tail.TailState()

    # -- reading ------------------------------------------------------------------

    def update(self, since: float | None = None) -> None:
        """Read what changed. ``since``: agent files last written before it and never read are
        skipped (they cannot change anything inside a done window that starts later; the main
        transcript is always read in full). Skipped files are read once they change."""
        self._children = None
        self._bg_index: dict[str, list[BgLaunch]] | None = None
        main = self.files.get(self.root) or self.files.setdefault(self.root, FileState(self.root, MAIN))
        agent_files, journals = discovery.related(self.root)
        for af in agent_files:
            rec = self.agents.setdefault(af.agent_id, AgentRec(af.agent_id))
            rec.file = af
            if not rec.meta:
                mtime = _mtime(af.meta)
                if mtime != rec.meta_mtime:  # an empty or half-written meta is retried when it changes
                    rec.meta, rec.meta_mtime = _read_meta(af.meta), mtime
            if af.transcript not in self.files:
                self.files[af.transcript] = FileState(af.transcript, af.agent_id)
        for run, path in journals.items():
            self.journals.setdefault(run, FileState(path, f"journal:{run}"))
            self._read_summary(run)
        reset = False
        skipped: list[FileState] = []
        for f in list(self.files.values()):
            if since is not None and f.owner != MAIN and f.tail.identity is None:
                try:
                    if os.path.getmtime(f.path) < since:
                        skipped.append(f)
                        continue
                except OSError:
                    continue
            reset |= self._read_file(f)
        for f in skipped:
            # an old file of an agent nobody saw end (no notice, no completed acknowledgement) can
            # still be waiting on its own background work (turn 8): read it after all when it
            # holds a background launch (a byte search, once per file version)
            ack = self.ack(f.owner)
            if f.owner in self.notices or (ack and ack.status == "completed"):
                continue
            key = (f.path, _mtime(f.path))
            if key in self._bg_scanned:
                continue
            self._bg_scanned.add(key)
            try:
                with open(f.path, "rb") as fh:
                    has_bg = BG_MARK in fh.read()
            except OSError:
                continue
            if has_bg:
                reset |= self._read_file(f)
        for run, j in self.journals.items():
            kind, lines = tail.read_appended(j.path, j.tail)
            if kind == tail.RESET:
                reset = True
                continue
            for line in lines:
                self._journal(run, wire.parse_line(line))
        self._read_hooks()
        if reset:  # a file was rewritten: everything derived from it is suspect; rebuild
            self._reset_stores()
            for f in self.files.values():
                if since is None or f.owner == MAIN or _mtime(f.path) >= since:
                    self._read_file(f)
            for run, j in self.journals.items():
                for line in tail.read_appended(j.path, j.tail)[1]:
                    self._journal(run, wire.parse_line(line))
                self._read_summary(run)
            self._read_hooks()  # the rebuild reset the hook stores too
        del main

    def _read_file(self, f: FileState) -> bool:
        kind, lines = tail.read_appended(f.path, f.tail)
        if kind == tail.RESET:
            return True
        if f.owner != MAIN:  # the agent's own calls: tool_use blocks in assistant records
            f.tool_calls += sum(raw.count(TOOL_USE) for raw in lines if ASSISTANT in raw)
        for i in wire.wanted_lines(lines):
            rec = wire.parse_line(lines[i])
            if rec is None:
                continue
            when = wire.ts(rec.get("timestamp")) or f.last_ts
            if when is not None:
                f.last_ts = when
            try:
                self.apply(f.owner, rec, when)
            except Exception:  # noqa: BLE001 - one odd record never costs the rest of the batch
                self.bad_records += 1
        if f.tail.size != f.info_size and f.tail.size > 0:
            f.info = summarize_tail(f.path, self.cwd)
            f.info_size = f.tail.size
        return False

    def _read_hooks(self) -> None:
        self._alive = {}  # process liveness is re-checked every update
        kind, lines = tail.read_appended(self.hook_file.path, self.hook_file.tail)
        if kind == tail.RESET:
            self.hook_starts, self.hook_stops, self.hook_procs = {}, {}, []
            kind, lines = tail.read_appended(self.hook_file.path, self.hook_file.tail)
        for raw in lines:
            rec = wire.parse_line(raw)
            if not rec or rec.get("v") != hookmod.VERSION or not isinstance(rec.get("agent"), str):
                continue
            when = rec.get("ts") if isinstance(rec.get("ts"), (int, float)) and not isinstance(rec.get("ts"), bool) else None
            if when is None:
                continue
            pid = rec.get("pid") if isinstance(rec.get("pid"), int) else None
            pstart = rec.get("pstart") if isinstance(rec.get("pstart"), str) else None
            pidns = rec.get("pidns") if isinstance(rec.get("pidns"), str) else None
            if pid is not None and pstart is not None:
                self.hook_procs.append((float(when), pid, pstart))
            if rec.get("event") == "start" and rec["agent"]:
                self.hook_starts.setdefault(rec["agent"], []).append((float(when), pid, pstart, pidns))
            elif rec.get("event") == "stop" and rec["agent"]:
                self.hook_stops.setdefault(rec["agent"], []).append(float(when))

    def process_gone(self, pid: int | None, start, pidns=None) -> bool:
        """Only a recorded, recognised Claude process in our pid namespace that no longer
        exists counts as gone; anything unknown is not."""
        if pid is None or not isinstance(start, str):
            return False
        key = (pid, start, pidns)
        if key not in self._alive:
            self._alive[key] = hookmod.process_alive(pid, start, pidns)
        return self._alive[key] is False

    def resumed_after(self, when: float, pid: int | None, start) -> bool:
        """A later hook record from another Claude process: the session was resumed (or
        restarted) since; an agent's missing stop then says nothing about how it ended."""
        return any(t > when and (p, s) != (pid, start) for t, p, s in self.hook_procs)

    def _read_summary(self, run: str) -> None:
        path = discovery.workflow_summary_path(self.root, run)
        try:
            mtime = os.path.getmtime(path)
        except OSError:
            return
        if self._wf_mtime.get(run) == mtime:
            return
        try:
            with open(path, encoding="utf-8") as fh:
                data = json.load(fh)
        except (OSError, ValueError):
            return  # mid-write: retried next tick
        if isinstance(data, dict):
            self.wf_summary[run] = data
            self._wf_mtime[run] = mtime

    def _journal(self, run: str, rec: dict | None) -> None:
        if not rec:
            return
        aid = rec.get("agentId")
        if rec.get("type") == "started" and aid:
            phase = rec.get("phase") if isinstance(rec.get("phase"), str) else None
            self.journal_started.setdefault(run, {})[aid] = (str(rec.get("label") or aid), phase)
            order = self.journal_phases.setdefault(run, [])
            if phase and phase not in order:
                order.append(phase)
        elif rec.get("type") == "result" and aid:
            self.journal_done.setdefault(run, set()).add(aid)
        elif rec.get("type") == "failed" and aid:
            self.journal_failed.setdefault(run, set()).add(aid)

    # -- the fold -------------------------------------------------------------------

    def apply(self, owner: str, rec: dict, when: float | None) -> None:
        kind = rec.get("type")
        notes = wire.parse_notices(wire.notice_text(rec) or "")
        if notes:
            for note in notes:
                if note["status"] and when is not None:
                    self.notices.setdefault(note["task"], set()).add((when, note["status"], note["tool_uses"]))
            return
        if kind == "continued-in":
            nxt = rec.get("continuedInSessionId")
            if owner == MAIN and isinstance(nxt, str) and nxt and nxt != self.session_id:
                self.continued_in = nxt
            return
        if kind == "custom-title" and isinstance(rec.get("customTitle"), str):
            self.titles["custom"] = rec["customTitle"]
        elif kind == "agent-name" and isinstance(rec.get("agentName"), str):
            self.titles["agent"] = rec["agentName"]
        elif kind == "ai-title" and isinstance(rec.get("aiTitle"), str):
            self.titles["ai"] = rec["aiTitle"]
        elif kind == "assistant":
            for b in wire.blocks(rec):
                if b.get("type") != "tool_use" or not b.get("id"):
                    continue
                name, inp = str(b.get("name") or ""), b.get("input") if isinstance(b.get("input"), dict) else {}
                if name in wire.SPAWN_TOOLS:
                    owners = self.spawn_owners.setdefault(b["id"], {})
                    owners[owner] = min(owners.get(owner, float("inf")), when if when is not None else float("inf"))
                    bg = inp.get("run_in_background")
                    old = self.spawns.get(b["id"])
                    if old is not None and not _better_owner(owner, when, old):
                        continue  # a fork's file repeats its parent's history, spawning calls included
                    self.spawns[b["id"]] = Spawn(b["id"], owner, when, str(inp.get("description") or ""),
                                                 inp.get("model") if isinstance(inp.get("model"), str) else None,
                                                 inp.get("subagent_type") if isinstance(inp.get("subagent_type"), str) else None,
                                                 bg is True or str(bg).lower() == "true")
                elif name.endswith("ask_operator"):
                    self._pending_ask[b["id"]] = owner
                elif name in ("Bash", "Monitor"):
                    self.bg_calls[b["id"]] = (name, str(inp.get("description") or ""))
                elif name in wire.BG_STOP_TOOLS:
                    task = inp.get("task_id") or inp.get("shell_id")
                    if isinstance(task, str) and task:
                        self.bg_stop_calls[b["id"]] = task
        elif kind == "user":
            if owner == MAIN and wire.is_human_prompt(rec) and when is not None:
                text = wire.string_content(rec) or ""
                first = next((ln.strip() for ln in text.splitlines() if ln.strip()), "")
                self.prompts.add((when, first[:200]))
            tur = rec.get("toolUseResult") if isinstance(rec.get("toolUseResult"), dict) else {}
            for b in wire.blocks(rec):
                if b.get("type") != "tool_result" or not b.get("tool_use_id"):
                    continue
                call = b["tool_use_id"]
                if call in self.spawns or tur.get("agentId"):
                    self.spawn_results.add(call)
                if tur.get("agentId") and tur.get("status"):
                    self.acks[call] = Ack(call, when, str(tur["agentId"]), str(tur["status"]),
                                          b.get("is_error") is True, tur.get("resolvedModel"),
                                          tur.get("totalToolUseCount") if isinstance(tur.get("totalToolUseCount"), int) else None)
                    self.ack_of[str(tur["agentId"])] = call
                if tur.get("runId"):
                    self.wf_launches[call] = WorkflowLaunch(call, when, str(tur["runId"]), tur.get("taskId"),
                                                            tur.get("workflowName"), tur.get("summary"))
                if isinstance(tur.get("resumedAgentId"), str) and tur.get("success") is not False and when is not None:
                    self.resumes.setdefault(str(tur["resumedAgentId"]), set()).add(when)
                self._background(owner, call, tur, b, when)
                if call in self._pending_ask:
                    self.qids.setdefault(self._pending_ask.pop(call), set()).update(
                        wire.question_ids(wire.text_of(b.get("content"))))

    def _background(self, owner: str, call: str, tur: dict, block: dict, when: float | None) -> None:
        task = tur.get("backgroundTaskId")
        kind = "Bash"
        timeout, persistent = None, False
        if not (isinstance(task, str) and task):
            task = tur.get("taskId")
            if not (isinstance(task, str) and task and "timeoutMs" in tur and "runId" not in tur):
                task = None
            else:
                kind = "Monitor"
                ms = tur.get("timeoutMs")
                timeout = ms / 1000.0 if isinstance(ms, (int, float)) and not isinstance(ms, bool) and ms > 0 else None
                persistent = tur.get("persistent") is True
        if task:
            old = self.bg.get(task)
            # a fork's file repeats its parent's history, background launches included: main
            # wins, then the earliest record
            if old is None or (old.owner != MAIN and (owner == MAIN or (when or float("inf")) < (old.ts or float("inf")))):
                self.bg[task] = BgLaunch(task, owner, when, kind, call, timeout, persistent)
        stop = self.bg_stop_calls.get(call)
        if not stop and isinstance(tur.get("task_id"), str) and "task_type" in tur:
            stop = tur["task_id"]  # TaskStop's own result names the task it stopped
        if stop and block.get("is_error") is not True and when is not None:
            self.bg_stops[stop] = min(when, self.bg_stops.get(stop, when))

    # -- queries --------------------------------------------------------------------

    def _bg_of(self, owner: str) -> list[BgLaunch]:
        """``owner``'s launches, indexed once per update (status queries ask per agent)."""
        if self._bg_index is None:
            self._bg_index = {}
            for launch in self.bg.values():
                self._bg_index.setdefault(launch.owner, []).append(launch)
        return self._bg_index.get(owner, [])

    def bg_description(self, launch: BgLaunch) -> str:
        return self.bg_calls.get(launch.call, ("", ""))[1]

    def open_bg(self, owner: str, now: float) -> list[BgLaunch]:
        """``owner``'s background tasks with no ending in the transcripts, oldest first."""
        out = []
        for launch in self._bg_of(owner):
            if launch.task in self.notices or launch.task in self.bg_stops:
                continue
            if (launch.kind == "Monitor" and not launch.persistent and launch.timeout and launch.ts is not None
                    and now > launch.ts + launch.timeout + MONITOR_SLACK):
                continue
            out.append(launch)
        return sorted(out, key=lambda x: x.ts or 0.0)

    def waiting_bg(self, owner: str, now: float) -> list[BgLaunch]:
        """The open tasks that are still running: their process lives (``bg_alive``), or, when
        that cannot be checked, they started at most ``BG_CAP`` ago."""
        out = []
        for launch in self.open_bg(owner, now):
            alive = self.bg_alive(launch)
            if alive is False or (alive is None and (launch.ts is None or now - launch.ts > BG_CAP)):
                continue
            out.append(launch)
        return out

    def last_activity(self) -> float | None:
        """The newest timestamp in any file read: a prompt, a reply, a tool call, a notice."""
        stamps = [t for f in self.files.values() for t in (f.info.last_ts, f.last_ts) if t is not None]
        return max(stamps, default=None)

    def file_of(self, agent_id: str) -> FileState | None:
        rec = self.agents.get(agent_id)
        return self.files.get(rec.file.transcript) if rec and rec.file else None

    def last_ts(self, agent_id: str) -> float | None:
        f = self.file_of(agent_id)
        if f is None:
            return None
        return max((t for t in (f.info.last_ts, f.last_ts) if t is not None), default=None)

    def parent_of(self, agent_id: str) -> str | None:
        """The agent that spawned this one (MAIN for the session), via ``meta.toolUseId``.

        A fork's own file repeats its history, including the call that spawned it, so the
        agent's own copy never counts. Of the other files holding the call, main wins, then
        the earliest. If only the agent's own copy was read (the parent's file can be skipped
        as old), ``meta.parentAgentId`` names the parent."""
        rec = self.agents.get(agent_id)
        call = (rec.meta.get("toolUseId") if rec else None) or self.ack_of.get(agent_id)
        owners = {o: t for o, t in self.spawn_owners.get(call or "", {}).items() if o != agent_id}
        if MAIN in owners:
            return MAIN
        if owners:
            return min(owners, key=lambda o: owners[o])
        parent = rec.meta.get("parentAgentId") if rec else None
        if isinstance(parent, str) and parent:
            return parent
        return MAIN if call in self.spawns else None

    def children(self, agent_id: str) -> list[str]:
        if self._children is None:
            self._children = {}
            for aid in self.agents:
                p = self.parent_of(aid)
                if p is not None:
                    self._children.setdefault(p, []).append(aid)
        return self._children.get(agent_id, [])

    def spawn_of(self, agent_id: str) -> Spawn | None:
        rec = self.agents.get(agent_id)
        call = (rec.meta.get("toolUseId") if rec else None) or self.ack_of.get(agent_id)
        return self.spawns.get(call or "")

    def ack(self, agent_id: str) -> Ack | None:
        call = self.ack_of.get(agent_id)
        if call is None:
            rec = self.agents.get(agent_id)
            call = rec.meta.get("toolUseId") if rec else None
        return self.acks.get(call or "")

    def run_end(self, run: str) -> tuple[float, str] | None:
        """(ts, status) of the latest notice for the run's latest launch, if terminal."""
        launches = sorted((w for w in self.wf_launches.values() if w.run_id == run), key=lambda w: w.ts or 0.0)
        if not launches:
            return None
        task = launches[-1].task_id
        notes = self.notices.get(task or "")
        if not notes:
            return None
        when, status, _ = max(notes)
        return when, status

    def run_live(self, run: str) -> bool | None:
        launches = [w for w in self.wf_launches.values() if w.run_id == run]
        if not launches:
            return None
        return self.run_end(run) is None

    def status_of(self, agent_id: str, now: float, _seen: frozenset = frozenset()) -> tuple[str, float | None, bool]:
        """(run | done | fail | cxl, ended_at, inferred). An agent that ended its turn silently
        but has a running descendant is waiting on it: running (review N2). Its own notice
        only arrives once no background child of its own is live, so that case never has
        terminal evidence."""
        status = self._status(agent_id, now)
        if status[0] == "done" and status[2] and agent_id not in _seen:
            seen = _seen | {agent_id}
            if any(self.status_of(c, now, seen)[0] in ("run", "bg") for c in self.children(agent_id) if c not in seen):
                return "run", None, True
        return status

    def _status(self, agent_id: str, now: float) -> tuple[str, float | None, bool]:
        rec = self.agents.get(agent_id)
        meta = rec.meta if rec else {}
        f = self.file_of(agent_id)
        last = self.last_ts(agent_id)
        run = rec.file.workflow if rec and rec.file else None
        evidence: list[tuple[float, str]] = []
        for when, status, _ in self.notices.get(agent_id, ()):
            evidence.append((when, status))
        ack = self.ack(agent_id)
        if ack and ack.status == "completed" and ack.ts is not None:
            evidence.append((ack.ts, "fail" if ack.is_error else "done"))
        if run and agent_id in self.journal_failed.get(run, set()):
            return "fail", last, False  # the run's own ledger: final
        if run and agent_id in self.journal_done.get(run, set()):
            return "done", last, False
        if run and agent_id not in self.journal_done.get(run, set()):
            end = self.run_end(run)
            if end is not None:
                evidence.append((end[0], "cxl" if end[1] in ("cxl", "fail") else end[1]))
            elif self.wf_summary.get(run, {}).get("status") in ("killed", "failed") and run_live_unknown(self, run):
                mt = self._wf_mtime.get(run)
                if mt is not None:
                    evidence.append((mt, "cxl"))
        notice_times = [t for t, _, _ in self.notices.get(agent_id, ())]
        for stop in self.hook_stops.get(agent_id, ()):
            # a recorded stop confirms an ending; a notice about the same stop says how it ended
            if not any(stop - REOPEN_MARGIN <= t <= stop + 60 for t in notice_times):
                evidence.append((stop, "done"))
        starts = self.hook_starts.get(agent_id, [])
        reopen_times = [*self.resumes.get(agent_id, ()), *(t for t, *_ in starts)]
        reopened = False
        if evidence:
            when, status = max(evidence)
            reopened = any(r > when for r in reopen_times) or (
                last is not None and last > when + REOPEN_MARGIN)
            if not reopened:
                return status, when, False
        if starts:
            hs, pid, pstart, pidns = max(starts, key=lambda x: x[0])
            if (not any(t >= hs for t, _ in evidence) and not self.resumed_after(hs, pid, pstart)
                    and self.process_gone(pid, pstart, pidns)):
                return "lost", last or hs, False  # started, never stopped, its Claude process is gone
        resumed_at = max(reopen_times, default=None)
        if resumed_at is not None and (last is None or resumed_at > last):
            last = resumed_at  # a resume restarts the clock before the agent writes anything
        # no standing terminal evidence: liveness
        if last is None:
            sp = self.spawn_of(agent_id)
            started = (ack.ts if ack else None) or (sp.ts if sp else None)
            if started is not None and now - started <= ASYNC_QUIET:
                return "run", None, True
            return "done", started, True
        if run and self.run_live(run) and last is not None:
            launches = [w.ts for w in self.wf_launches.values() if w.run_id == run and w.ts is not None]
            first = f.info.first_ts if f else None
            if launches and first is not None and first < max(launches) - REOPEN_MARGIN:
                # started by an earlier instance of a run that was relaunched (resumed): the new
                # instance re-runs unfinished steps with new agents; this one was abandoned
                return "cxl", max(launches), False
            if now - last <= PENDING_CAP:
                # the live run still waits on it (the same ground truth as an open spawning call)
                return "run", None, False
        sp = self.spawn_of(agent_id)
        if sp is not None and not sp.background and sp.call not in self.spawn_results:
            # foreground: the parent still waits on the spawning call; that is ground truth
            if now - max(t for t in (last, sp.ts) if t is not None) <= PENDING_CAP:
                return "run", None, False
        pending = f.info.pending if f else []
        pend_ok = any(p_ts is not None and now - p_ts <= PENDING_CAP for _, _, p_ts in pending)
        if now - last <= ACTIVE or pend_ok:
            return "run", None, False
        if self.waiting_bg(agent_id, now):
            return "bg", None, False  # its turn ended, its background work is still running
        if meta.get("agentType") == "fork":
            return "done", last, True  # interactive sidechain: idle
        background = (ack is not None and ack.status == "async_launched") or meta.get("requestShape") == "background"
        if (background or reopened) and (not evidence or reopened) and now - last <= ASYNC_QUIET:
            return "run", None, True  # not notified yet: still working (a long model turn writes nothing)
        if f and f.info.handback:
            return "done", last, False
        return "done", last, True

    def prompt_before(self, when: float | None) -> tuple[int, str] | None:
        """zoetrope ``prompt_for_ts``: the latest human prompt at or before ``when`` (index, text)."""
        if when is None:
            return None
        ordered = sorted(self.prompts)
        best = None
        for i, (t, text) in enumerate(ordered):
            if t <= when:
                best = (i, text)
            else:
                break
        return best


def _better_owner(owner: str, when: float | None, old: Spawn) -> bool:
    """Order-independent choice when one spawning call appears in several files: main wins,
    then the earliest record (the original, not a fork's copy)."""
    if old.owner == MAIN:
        return False
    if owner == MAIN:
        return True
    return (when or float("inf")) < (old.ts or float("inf"))


def run_live_unknown(sess: Session, run: str) -> bool:
    """No Workflow launch record for the run is known (compacted away): trust the run JSON."""
    return sess.run_live(run) is None


def _mtime(path: str) -> float:
    try:
        return os.path.getmtime(path)
    except OSError:
        return 0.0


def _read_meta(path: str) -> dict:
    try:
        with open(path, encoding="utf-8") as fh:
            data = json.load(fh)
    except (OSError, ValueError):
        return {}  # missing or mid-write: retried next update
    return data if isinstance(data, dict) else {}


def summarize_tail(path: str, cwd: str | None, window: int = TAIL_BYTES) -> TailInfo:
    """Activity, in-flight calls, last text, model and timestamps from the end of the file
    (+ first line). The window starts at 64 KB and doubles, up to 8 MB, until it holds at
    least two dated user/assistant records, so a huge last record cannot hide them."""
    info = TailInfo()
    try:
        size = os.path.getsize(path)
        with open(path, "rb") as fh:
            head = fh.readline(262144)
            while True:
                start = max(0, size - window)
                fh.seek(start)
                data = fh.read()
                dated = data.count(b'"type":"assistant"') + data.count(b'"type":"user"')
                if start == 0 or window >= TAIL_MAX or (dated >= 2 and data.find(b"\n") < len(data) // 2):
                    break
                window *= 2
    except OSError:
        return info
    first = wire.parse_line(head) if head.endswith(b"\n") else None
    info.first_ts = wire.ts((first or {}).get("timestamp"))
    lines = data.split(b"\n")
    if start > 0:
        lines = lines[1:]
    uses: dict[str, tuple[str, dict, float | None]] = {}
    done: set[str] = set()
    last_use = None
    for raw in lines:
        if not raw.strip():
            continue
        rec = wire.parse_line(raw)
        if rec is None:
            continue
        when = wire.ts(rec.get("timestamp"))
        if when is not None and rec.get("type") in ("user", "assistant", "system"):
            info.last_ts = when
        if rec.get("type") == "assistant":
            msg = rec.get("message") if isinstance(rec.get("message"), dict) else {}
            if isinstance(msg.get("model"), str) and not msg["model"].startswith("<"):
                info.model = msg["model"]
        for b in wire.blocks(rec):
            t = b.get("type")
            if rec.get("type") == "assistant" and t == "tool_use" and b.get("id"):
                inp = b.get("input") if isinstance(b.get("input"), dict) else {}
                uses[b["id"]] = (str(b.get("name") or "?"), inp, when)
                last_use = b["id"]
                if b.get("name") == "SubagentHandback":
                    info.handback = True
            elif rec.get("type") == "assistant" and t == "text" and str(b.get("text") or "").strip():
                info.last_text = str(b["text"]).strip()
            elif t == "tool_result" and b.get("tool_use_id"):
                done.add(b["tool_use_id"])
    info.pending = [v for k, v in uses.items() if k not in done]
    current = info.pending[-1] if info.pending else (uses.get(last_use) if last_use else None)
    if current:
        info.activity = wire.summarize_tool(current[0], current[1], cwd)
    return info
