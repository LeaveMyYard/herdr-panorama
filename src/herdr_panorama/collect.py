"""Build the Swarm model: herdr's snapshot (which sessions are live, blocked) + one
``scan.Session`` per Claude session (agents, statuses, nesting) + thread grouping
+ the explicit thread channel + herdr-inbox questions.

A ``Collector`` lives as long as the view; each ``collect`` stats unchanged files
and reads only appended bytes of changed ones.
"""

from __future__ import annotations

import os
import re
import time

from . import bgproc, config, events, herdr, threads
from .model import (BG, BLK, CXL, DONE, FAIL, INF, LOST, REV, RUN, WAIT, Agent, BgTask, Orchestrator, Phase, Swarm,
                    Thread, live)
from .scan import discovery
from .scan.session import MAIN, BgLaunch, Session

NO_PID_RETRY = 30.0  # seconds before asking herdr again for a pane's Claude process
QUIET_AFTER = 30 * 60  # a running agent silent this long reads "quiet 42m"
INBOX_MARK = re.compile(r"^(?:❓\d+\s+)+")  # herdr-inbox's tab-label prefix; the card shows ?N itself


_MODEL_AFTER = re.compile(r"(opus|sonnet|haiku|fable)[-_ ]?(\d{1,2})(?:[-_.](\d{1,2}))?(?!\d)")
_MODEL_BEFORE = re.compile(r"(?<!\d)(\d{1,2})(?:[-_.](\d{1,2}))?[-_ ](opus|sonnet|haiku|fable)")


def short_model(model: str | None) -> str | None:
    """The model's family and version: "claude-opus-4-5-20251101" -> "opus 4.5",
    "claude-3-5-sonnet-20241022" -> "sonnet 3.5"; the family alone when there is no version
    (design turn 7: wide cards show the version)."""
    if not model:
        return None
    low = model.lower()
    m = _MODEL_AFTER.search(low)
    if m:
        return f"{m.group(1)} {m.group(2)}" + (f".{m.group(3)}" if m.group(3) else "")
    m = _MODEL_BEFORE.search(low)
    if m:
        return f"{m.group(3)} {m.group(1)}" + (f".{m.group(2)}" if m.group(2) else "")
    for name in ("opus", "sonnet", "haiku", "fable"):
        if name in low:
            return name
    return model


def dur(seconds: float) -> str:
    m = int(max(0, seconds) // 60)
    return f"{m // 60}h{m % 60:02d}m" if m >= 60 else f"{m}m"


def _int(value, default: int) -> int:
    try:
        return int(value)
    except (TypeError, ValueError, OverflowError):
        return default


def _paragraph(text: str, limit: int = 400) -> str:
    para = " ".join((text or "").strip().split("\n\n")[0].split())
    return para if len(para) <= limit else para[: limit - 1] + "…"


class Collector:
    def __init__(self, root: str | None = None, now=time.time, done_window: float | None = None,
                 procs: bool = True, pane_pid=herdr.claude_pid, stale_after: float | None = None):
        """``done_window`` / ``stale_after`` fixed for tests; by default they follow the plugin
        config (re-read per collect). ``procs``: background tasks can be checked against this
        machine's processes (False for another machine's mirrored transcripts)."""
        self.root = root or discovery.projects_root()
        self.now = now
        self.fixed_window = done_window
        self.window = done_window if done_window is not None else config.done_window()
        self.fixed_stale = stale_after
        self.procs, self.pane_pid = procs, pane_pid
        self._pids: dict[str, tuple[int, str | None]] = {}  # pane -> its Claude process (pid, start)
        self._no_pid: dict[str, float] = {}  # pane -> when herdr last could not name its Claude process
        self.sessions: dict[str, Session] = {}
        self.channels: dict[str, events.Channel] = {}
        self.self_pane: str | None = None  # the view's own pane (cockpit: which agent it is docked by)

    def session(self, session_id: str, cwd: str | None) -> Session | None:
        sess = self.sessions.get(session_id)
        if sess is None or not os.path.isfile(sess.root):
            path = discovery.find_root(session_id, cwd, self.root)
            if not path:
                return None
            sess = self.sessions[session_id] = Session(session_id, path, cwd)
        sess.update(since=self.now() - self.window - 600)
        return sess

    # -- the model ------------------------------------------------------------------

    def collect(self, snapshot: dict | None, open_qs: list | None) -> Swarm:
        now = self.now()
        if self.fixed_window is None:
            self.window = config.done_window()
        swarm = Swarm(generated_at=now, herdr_ok=snapshot is not None, inbox_ok=open_qs is not None,
                      stale_after=self.fixed_stale if self.fixed_stale is not None else config.stale_after())
        if not snapshot:
            return swarm
        tabs = {t.get("tab_id"): t for t in snapshot.get("tabs") or [] if isinstance(t, dict)}
        order = {tid: i for i, tid in enumerate(tabs)}
        agents = [a for a in snapshot.get("agents") or [] if isinstance(a, dict) and a.get("agent") == "claude"]
        per_tab: dict = {}
        for a in agents:
            per_tab[a.get("tab_id")] = per_tab.get(a.get("tab_id"), 0) + 1
        agents.sort(key=lambda a: (order.get(a.get("tab_id"), 999), a.get("pane_id") or ""))
        tab_no: dict = {}
        for t in tabs.values():  # herdr's tab number: the position in its workspace, 1-based
            ws_tabs = [x for x in tabs.values() if x.get("workspace_id") == t.get("workspace_id")]
            tab_no[t.get("tab_id")] = ws_tabs.index(t) + 1
        me = next((p for p in snapshot.get("panes") or [] if isinstance(p, dict) and self.self_pane
                   and p.get("pane_id") == self.self_pane), None)
        for a in agents:
            if me and a.get("tab_id") == me.get("tab_id") and swarm.docked is None:
                swarm.docked = a.get("pane_id")  # cockpit: the agent panorama is docked next to
        for a in agents:
            try:
                swarm.orchestrators.append(self._orchestrator(a, tabs, per_tab, open_qs or [], now))
                swarm.orchestrators[-1].tab_no = tab_no.get(a.get("tab_id"))
            except Exception as exc:  # noqa: BLE001 - one broken session never hides the others
                swarm.errors.append(f"{a.get('pane_id')}: {type(exc).__name__}: {exc}"[:300])
        swarm.errors += [f"{sid[:8]}: {s.bad_records} unreadable records" for sid, s in self.sessions.items()
                         if s.bad_records]
        return swarm

    def _orchestrator(self, a: dict, tabs: dict, per_tab: dict, open_qs: list, now: float) -> Orchestrator:
        tab = tabs.get(a.get("tab_id")) or {}
        sess_info = a.get("agent_session") if isinstance(a.get("agent_session"), dict) else {}
        sid = sess_info.get("value") if sess_info.get("kind", "id") == "id" else None
        orch = Orchestrator(pane_id=a.get("pane_id") or "", tab_id=a.get("tab_id"), tab_label="",
                            session_id=sid, herdr_status=a.get("agent_status") or "unknown",
                            workspace_id=a.get("workspace_id"), focused=bool(a.get("focused")),
                            cwd=a.get("cwd") if isinstance(a.get("cwd"), str) else None)
        mine = [q for q in open_qs if (sid and q.session_id == sid) or (not q.session_id and q.pane_id == orch.pane_id)]
        orch.inbox, orch.inbox_texts = [q.id for q in mine], [q.question for q in mine]
        sess = self.session(sid, a.get("cwd")) if sid else None
        titles = sess.titles if sess else {}
        label = (INBOX_MARK.sub("", tab.get("label") or "") or titles.get("custom") or titles.get("agent") or titles.get("ai")
                 or a.get("terminal_title_stripped") or orch.pane_id or "?")
        if per_tab.get(a.get("tab_id"), 0) > 1:
            label = f"{label} · {orch.pane_id}"
        orch.tab_label = str(label)
        if sess is None:
            return orch
        orch.transcript = sess.root
        sess.bg_alive = self._bg_alive(orch.pane_id)
        orch.bg = bg_tasks(sess, sess.waiting_bg(MAIN, now))
        orch.last_activity = sess.last_activity()
        main_file = sess.files.get(sess.root)
        if orch.herdr_status == "blocked" and main_file is not None:
            orch.pending = pending_prompt(main_file.info.pending)
        qtext = {q.id: q.question for q in mine}
        orch.threads, orch.hidden_done = self._threads(sess, qtext, now)
        orch.flavour = flavour(orch.threads)
        card = main_session_thread(orch, sess)
        if card is not None:
            orch.threads.insert(0, card)
        return orch

    # -- background work --------------------------------------------------------------

    def _bg_alive(self, pane_id: str):
        """A ``Session.bg_alive`` for the pane's session: a background Bash is alive while a
        descendant of the pane's Claude process holds its output file (bgproc.py). The
        process tree is read at most once per collect, and only when a task is open."""
        cache: dict = {}

        def alive(launch: BgLaunch) -> bool | None:
            if not self.procs or launch.kind != "Bash":
                return None  # a Monitor's process is not visible this way: its timeout decides
            if "tasks" not in cache:
                cache["tasks"] = self._live_tasks(pane_id)
            tasks = cache["tasks"]
            return None if tasks is None else launch.task in tasks
        return alive

    def _live_tasks(self, pane_id: str) -> set[str] | None:
        pid, start = self._pids.get(pane_id, (None, None))
        if pid is None or bgproc.start_token(pid) != start:
            if self._no_pid.get(pane_id, float("-inf")) > self.now() - NO_PID_RETRY:
                return None  # herdr could not name it a moment ago: do not ask on every collect
            pid = self.pane_pid(pane_id)
            if pid is None:
                self._pids.pop(pane_id, None)
                self._no_pid[pane_id] = self.now()
                return None
            start = bgproc.start_token(pid)
            self._pids[pane_id] = (pid, start)
        return bgproc.live_tasks(pid)

    # -- agents ---------------------------------------------------------------------

    def _agents(self, sess: Session, qtext: dict[int, str], now: float) -> dict[str, Agent]:
        items = sorted(sess.agents.items())
        out: dict[str, Agent] = {}
        old: dict[str, int] = {}  # finished long ago (outside the done window) -> seq
        for seq, (aid, rec) in enumerate(items):
            self._agent(sess, qtext, now, seq, aid, rec, out, old)
        # an agent outside the window still shows while an agent it spawned does: subagents nest
        # at any depth, and a tree never loses the agents between a shown one and its thread
        for aid in list(out):
            cur, seen = sess.parent_of(aid), {aid}
            while cur not in (None, MAIN) and cur not in seen:
                seen.add(cur)
                if cur in old and cur not in out:
                    self._agent(sess, qtext, now, old[cur], cur, sess.agents[cur], out, None)
                cur = sess.parent_of(cur)
        return dict(sorted(out.items(), key=lambda kv: kv[1].seq))

    def _agent(self, sess: Session, qtext: dict[int, str], now: float, seq: int, aid: str, rec,
               out: dict[str, Agent], old: dict[str, int] | None) -> None:
        """Build one agent into ``out``; one finished outside the done window goes to ``old``
        instead (``old`` None: build it anyway)."""
        f = sess.file_of(aid)
        status, ended, inferred = sess.status_of(aid, now)
        if status == "bg":
            status = BG
        elif status == DONE and inferred:
            status = INF  # never shown as a confirmed done (issue #2)
        elif status == "lost":
            status = LOST
        last = sess.last_ts(aid)
        if old is not None and not live(status) and (ended or last or 0) < now - self.window:
            old[aid] = seq
            return
        sp, ack = sess.spawn_of(aid), sess.ack(aid)
        meta = rec.meta
        ag = Agent(id=aid, description=str(meta.get("description") or (sp.description if sp else "")),
                   agent_type=meta.get("agentType") or (sp.subagent_type if sp else None),
                   model=short_model(meta.get("model") or (ack.model if ack else None)
                                     or (f.info.model if f else None) or (sp.model if sp else None)),
                   status=status, tool_use_id=meta.get("toolUseId") or (sp.call if sp else None),
                   started=(sp.ts if sp else None) or (f.info.first_ts if f else None),
                   ended=ended if not live(status) else None, last_at=last, inferred=inferred,
                   transcript=rec.file.transcript if rec.file else None, seq=seq,
                   resumes=len(sess.resumes.get(aid, ())), depth=_int(meta.get("spawnDepth"), 1))
        counted = max((n for n in sess.notices.get(aid, ()) if n[2] is not None), default=None)
        ag.tool_calls = max(f.tool_calls if f else 0, counted[2] if counted else 0,
                            (ack.tool_count or 0) if ack else 0)
        parent = sess.parent_of(aid)
        ag.parent = parent if parent not in (None, MAIN) else None
        if f:
            ag.activity = f.info.activity
            ag.last_message = _paragraph(f.info.last_text)
        ag.question_ids = sorted(q for q in sess.qids.get(aid, ()) if q in qtext)
        ag.questions = [qtext[q] for q in ag.question_ids]
        if status == FAIL:
            ag.error = (ag.last_message or "failed").splitlines()[0][:200]
        if status == BG:
            ag.bg = bg_tasks(sess, sess.waiting_bg(aid, now))
            n = len(ag.bg)
            ag.activity = f"turn ended · {n} task{'s' if n != 1 else ''} in background"
        if live(status) and ag.question_ids:
            ag.status = WAIT
        run = rec.file.workflow if rec.file else None
        label = sess.journal_started.get(run or "", {}).get(aid, ("", None))[0]
        ag.role = threads.role_of(label, ag.description, fallback=ag.agent_type)
        if ag.status == RUN and ag.role == "reviewer":
            ag.status = REV
        if live(ag.status) and ag.status != BG and last and now - last > QUIET_AFTER:
            ag.activity = f"quiet {dur(now - last)}" + (f" · {ag.activity}" if ag.activity else "")
        out[aid] = ag

    # -- threads --------------------------------------------------------------------

    def _threads(self, sess: Session, qtext: dict[int, str], now: float) -> tuple[list[Thread], int]:
        by_id = self._agents(sess, qtext, now)
        claimed: set[str] = set()
        out: list[Thread] = []
        channel = self.channels.get(sess.session_id)
        if channel is None:
            channel = self.channels[sess.session_id] = events.Channel(sess.session_id)
        channel.update()
        for d in sorted(channel.threads.values(), key=lambda d: d.order):
            out.append(declared_thread(d, by_id, claimed))
        runs = sorted({r.file.workflow for r in sess.agents.values() if r.file and r.file.workflow}
                      | {w.run_id for w in sess.wf_launches.values()})
        for run in runs:
            t = workflow_thread(sess, run, by_id, claimed)
            if t is not None:
                out.append(t)
        top = [a for a in by_id.values() if a.id not in claimed and not a.parent]
        turn_of: dict[str, tuple[int, str]] = {}
        spans: dict[str, list[tuple]] = {}
        for a in top:
            p = sess.prompt_before(a.started)
            if p is not None:
                turn_of[a.id] = p
            spans[a.id] = agent_spans(sess, a)
        out += threads.group_refs_and_turns(top, turn_of, spans)
        where = {a.id: t for t in out for a in t.agents}

        def chain(a: Agent) -> list[Agent]:
            """``a`` and the agents that spawned it, up to its top one (any depth, loop-safe)."""
            out_, seen = [a], {a.id}
            while out_[-1].parent in by_id and out_[-1].parent not in seen:
                out_.append(by_id[out_[-1].parent])
                seen.add(out_[-1].id)
            return out_

        # parents first (so a nested agent can take its parent's phase), by the spawn chain
        for a in sorted(by_id.values(), key=lambda a: (len(chain(a)), a.seq)):
            if a.id in where:
                continue
            root = chain(a)[-1]
            t = where.get(root.id)
            if t is None:
                t = Thread(id=f"orphan:{root.id}", name=root.description or root.id, source="turn", agents=[], status="")
                out.append(t)
                where[root.id] = t
            t.agents.append(a)
            parent = by_id.get(a.parent or "")
            if parent and parent.phases:
                a.phases = [parent.phases[-1]]
            where[a.id] = t
        kept, hidden = [], 0
        for t in out:
            threads.finish(t)
            if (t.done or t.status == "failed") and (t.ended or 0) < now - self.window:
                hidden += 1
                continue
            kept.append(t)
        return threads.sort_threads(kept), hidden


def bg_tasks(sess: Session, launches: list[BgLaunch]) -> list[BgTask]:
    return [BgTask(x.task, x.kind, sess.bg_description(x), x.ts) for x in launches]


def agent_spans(sess: Session, a: Agent) -> list[tuple[float | None, float | None, str]]:
    """(start, end, outcome) per launch and per resume, from the session's notices and resumes."""
    starts = sorted({t for t in [a.started, *sess.resumes.get(a.id, ())] if t is not None})
    ends = sorted((t, st) for t, st, _ in sess.notices.get(a.id, ()))
    out = []
    for i, s in enumerate(starts):
        nxt = starts[i + 1] if i + 1 < len(starts) else float("inf")
        end = next(((t, st) for t, st in ends if s <= t < nxt), None)
        if end:
            out.append((s, end[0], end[1]))
        elif i + 1 < len(starts):
            out.append((s, nxt, DONE))
        else:
            out.append((s, a.ended if not live(a.status) else None, a.status if not live(a.status) else RUN))
    return out


def workflow_thread(sess: Session, run: str, by_id: dict[str, Agent], claimed: set[str]) -> Thread | None:
    members = [aid for aid, r in sess.agents.items() if r.file and r.file.workflow == run]
    ags = [by_id[aid] for aid in members if aid in by_id and aid not in claimed]
    live_run = sess.run_live(run)
    if not ags and not live_run:
        return None
    claimed.update(a.id for a in ags)
    started = sess.journal_started.get(run, {})
    summary = sess.wf_summary.get(run, {})
    phase_of = {}
    for a in ags:
        meta_phase = sess.agents[a.id].meta.get("workflowPhase")
        phase_of[a.id] = started.get(a.id, ("", None))[1] or (meta_phase if isinstance(meta_phase, str) else None)
    order = [str(p["title"]) for p in summary.get("phases") or [] if isinstance(p, dict) and p.get("title")]
    for ph in sess.journal_phases.get(run, []) + [p for p in phase_of.values() if p]:
        if ph not in order:
            order.append(ph)
    detail = {str(p["title"]): str(p.get("detail") or "") for p in summary.get("phases") or []
              if isinstance(p, dict) and p.get("title")}
    launches = sorted((w for w in sess.wf_launches.values() if w.run_id == run), key=lambda w: w.ts or 0.0)
    name = next((w.name for w in reversed(launches) if w.name), None) or summary.get("workflowName") or run
    end = sess.run_end(run)
    run_is_live = live_run if live_run is not None else any(live(a.status) for a in ags)
    t = Thread(id=f"wf:{run}", name=str(name), source="workflow", agents=ags, flavour="workflow",
               phases=threads.workflow_phases(order, ags, phase_of, bool(run_is_live), detail))
    if live_run is None:
        t.status = ""
    elif live_run:
        t.status = "active"
    else:
        t.status = {DONE: "done", FAIL: "failed", CXL: "cancelled"}.get(end[1], "done") if end else "done"
        t.ended = end[0] if end else None
    t.started = launches[0].ts if launches else None
    return t


def declared_thread(d: events.Declared, by_id: dict[str, Agent], claimed: set[str]) -> Thread:
    phases = [Phase(str(p["name"]), p.get("state") if p.get("state") in events.STATES else "next",
                    events.phase_time(p.get("start"), d.ts), events.phase_time(p.get("end"), d.ts),
                    str(p["note"]) if p.get("note") else None) for p in d.phases]
    ags = []
    for spec in d.agents:
        for a in by_id.values():
            if a.id in claimed or not events.matches(spec["match"], a.id, a.description):
                continue
            claimed.add(a.id)
            if isinstance(spec.get("role"), str):
                a.role = spec["role"]
            if isinstance(spec.get("phase"), int) and 0 <= spec["phase"] < len(phases):
                a.phases = [spec["phase"]]
            ags.append(a)
    status = d.status if d.status in ("active", "done", "failed", "cancelled") else ""
    return Thread(id=f"x:{d.thread}", name=d.name or d.thread, source="explicit", phases=phases, agents=ags,
                  status=status, worktree=d.worktree, pct=d.pct, flavour=d.flavour)


def flavour(ts: list[Thread]) -> str:
    for t in ts:
        if t.source == "explicit" and t.flavour:
            return t.flavour
    if any(t.source == "workflow" for t in ts):
        return "workflow"
    if any(t.source == "ref" for t in ts):
        return "pr-review"
    return "agents"


def pending_prompt(pending: list) -> list[str]:
    """What a blocked session waits on: AskUserQuestion texts, or ``wants Tool(...)``."""
    from .scan.wire import summarize_tool
    if not pending:
        return []
    name, inp, _ = pending[-1]
    if name == "AskUserQuestion" and isinstance(inp, dict):
        qs = [str(q.get("question") or "") for q in inp.get("questions") or [] if isinstance(q, dict)]
        return [q for q in qs if q]
    return ["wants " + summarize_tool(name, inp)]


def main_session_thread(orch: Orchestrator, sess: Session) -> Thread | None:
    """A card for the orchestrator itself when it needs the operator: a blocking prompt
    (herdr ``blocked``) or inbox questions no subagent filed."""
    attributed = {q for t in orch.threads for a in t.agents for q in a.question_ids}
    loose = [(qid, text) for qid, text in zip(orch.inbox, orch.inbox_texts) if qid not in attributed]
    blocked = orch.herdr_status == "blocked"
    if not blocked and not loose:
        return None
    since = max((t for t, _ in sess.prompts), default=None)
    ag = Agent(id=f"{MAIN}:{orch.pane_id}", description="main session", role="orchestrator",
               status=BLK if blocked else WAIT, started=since, phases=[0])
    ag.question_ids, ag.questions = [q for q, _ in loose], [t for _, t in loose]
    if blocked:
        ag.activity = "waiting for you in the pane"
        ag.pending = list(orch.pending)
    return Thread(id=f"{MAIN}:{orch.pane_id}", name=orch.tab_label, source="session",
                  phases=[Phase("waiting on you", "now", since)], agents=[ag], status="active", started=since)
