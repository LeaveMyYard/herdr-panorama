"""What the operator has not seen yet (design turn 6, "Swarm View 6 unseen").

A new failure, a new question or a finished agent marks that agent *unseen* until the
operator clicks it, presses Enter on it, docks next to its orchestrator, or focuses the
orchestrator's pane in herdr. The view pulses unseen cards (render.py).

The state lives in one JSON file under panorama's state directory, shared by every panorama
process of the machine (popup, home tab, docked panel) and kept across restarts:

    {"v": 1, "saved": <time>, "agents": {"<agent id>": {"s": <status>, "q": [<question ids>],
                                                       "ev": "fail"|"wait"|"done"|null, "t": <last seen alive>}}}

Each refresh compares the swarm with the file under a short lock: a status change or a new
question id since the file's record is an event, so two processes never report one event
twice, and a mark cleared in one process is cleared in all. The first run (no file) only
records what is there: nothing that happened before panorama looked is "new".
"""

from __future__ import annotations

import json
import logging
import os
import threading
import time
from pathlib import Path

from . import config, home
from .model import CXL, DONE, FAIL, INF, WAIT, Swarm

log = logging.getLogger("herdr_panorama.unseen")

RANK = {"fail": 0, "wait": 1, "done": 2}  # most urgent first
FINISHED = (DONE, INF)
KEEP_SECONDS = 7 * 86400  # forget agents not seen for a week
LOCK_SECONDS = 0.2


def enabled() -> bool:
    value = config.load().get("highlight_unseen", True)
    return value if isinstance(value, bool) else True


def urgent(a: str | None, b: str | None) -> str | None:
    if a is None:
        return b
    if b is None:
        return a
    return a if RANK[a] <= RANK[b] else b


def default_path() -> Path:
    return home.state_dir() / f"unseen-{home.session_key()}.json"


class Tracker:
    def __init__(self, path: Path | None = None, enabled_fn=enabled):
        self.path = path
        self.enabled_fn = enabled_fn
        self.marks: dict[str, str] = {}  # agent id -> its most urgent unseen event
        self.pending: set[str] = set()  # seen here, not yet written (the lock was busy)
        self.prev_focused: set[str] | None = None  # orchestrator panes herdr had focused at the last update
        self.keymap: dict[str, str] = {}  # agent id -> its state file key (machine-prefixed if remote)
        self._mu = threading.Lock()  # marks / pending: the collect worker and the UI thread

    def _path(self) -> Path:
        return self.path or default_path()

    # -- file -----------------------------------------------------------------------

    def _read(self) -> dict | None:
        try:
            data = json.loads(self._path().read_text())
        except FileNotFoundError:
            return None
        except (OSError, ValueError) as exc:
            log.warning("unseen state unreadable, starting over: %s", exc)
            return None
        if not isinstance(data, dict) or not isinstance(data.get("agents"), dict):
            return None
        return data

    def _write(self, data: dict) -> None:
        p = self._path()
        p.parent.mkdir(parents=True, exist_ok=True)
        tmp = p.with_name(p.name + f".{os.getpid()}.tmp")
        tmp.write_text(json.dumps(data, separators=(",", ":")))
        os.replace(tmp, p)

    def _locked(self, fn):
        """Run ``fn`` under the state file's lock; None when another process holds it for longer
        than LOCK_SECONDS (this tick keeps the last marks)."""
        import fcntl
        p = self._path()
        p.parent.mkdir(parents=True, exist_ok=True)
        fd = os.open(str(p) + ".lock", os.O_WRONLY | os.O_CREAT, 0o644)
        try:
            end = time.monotonic() + LOCK_SECONDS
            while True:
                try:
                    fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    break
                except BlockingIOError:
                    if time.monotonic() >= end:
                        return None
                    time.sleep(0.01)
            try:
                return fn()
            finally:
                fcntl.flock(fd, fcntl.LOCK_UN)
        finally:
            os.close(fd)

    # -- the model --------------------------------------------------------------------

    @staticmethod
    def key(o, a) -> str:
        """The state file's key: an agent on another herdr machine carries that machine's id."""
        mid = getattr(o, "machine_id", None) or getattr(o, "machine", None)
        return f"{mid}:{a.id}" if mid else a.id

    def update(self, swarm: Swarm, now: float) -> dict[str, str]:
        """Record this refresh's agents; returns the unseen marks (agent id -> event). Runs on
        the collect worker: file I/O and the lock wait stay off the UI thread."""
        if not self.enabled_fn():
            with self._mu:
                self.marks = {}
            return {}
        try:
            marks = self._locked(lambda: self._update(swarm, now))
        except OSError as exc:
            log.warning("unseen state not saved: %s", exc)
            marks = None
        with self._mu:
            if marks is not None:
                self.marks = marks
            return dict(self.marks)

    def _update(self, swarm: Swarm, now: float) -> dict[str, str]:
        data = self._read()
        first = data is None
        if first:
            data = {"v": 1, "saved": now, "agents": {}}
        agents: dict = data["agents"]
        saved = float(data.get("saved") or 0.0)
        # herdr's focus clears an orchestrator's marks only when it MOVES onto that pane, and only
        # marks that arose before: focus resting on a pane (the operator away, the cockpit's docked
        # agent) is not looking, and the first refresh after start has nothing to compare with
        focused = {o.pane_id for o in swarm.orchestrators if o.focused and not o.machine}
        arrived = set() if self.prev_focused is None else focused - self.prev_focused
        self.prev_focused = focused
        with self._mu:
            pending = set(self.pending)
        keymap: dict[str, str] = {}
        for o in swarm.orchestrators:
            for a in o.agents():
                if a.id.startswith("main:"):
                    continue
                k = keymap[a.id] = self.key(o, a)
                rec = agents.get(k)
                ev = None
                if rec is None:
                    if not first:  # appeared since the last look: only what happened after it counts
                        when = a.ended or a.last_at or a.started or 0.0
                        if when >= saved:
                            ev = self._event(None, [], a)
                    rec = agents[k] = {"s": a.status, "q": list(a.question_ids), "ev": None, "t": now}
                else:
                    ev = self._event(rec.get("s"), rec.get("q") or [], a)
                prev = rec.get("ev")
                if prev == "wait" and not a.question_ids and a.status != WAIT:
                    prev = None  # answered: the operator saw it where they answered
                if o.pane_id in arrived or a.id in pending:
                    prev = None  # focus moved onto its orchestrator's pane, or seen here
                rec["ev"] = urgent(prev, ev)
                rec["s"], rec["q"], rec["t"] = a.status, sorted(set(rec.get("q") or []) | set(a.question_ids)), now
        for k in [k for k, r in agents.items() if now - float(r.get("t") or 0) > KEEP_SECONDS]:
            del agents[k]
        data["saved"] = now
        self._write(data)
        with self._mu:
            self.pending -= pending
            self.keymap = keymap
        return {aid: agents[k]["ev"] for aid, k in keymap.items() if agents[k].get("ev") in RANK}

    @staticmethod
    def _event(prev_status: str | None, prev_qs: list, a) -> str | None:
        """The event between the recorded state and this one (None: no record yet)."""
        ev = None
        if a.status == FAIL and prev_status != FAIL:
            ev = urgent(ev, "fail")
        if set(a.question_ids) - set(prev_qs):
            ev = urgent(ev, "wait")
        if a.status in FINISHED and prev_status not in FINISHED and prev_status != CXL:
            ev = urgent(ev, "done")
        return ev

    def seen(self, ids) -> set[str]:
        """The operator looked at these agents: their marks clear here at once; returns the ids
        whose mark cleared, for ``save_seen`` (file I/O: call it off the UI thread)."""
        with self._mu:
            ids = {i for i in ids if i in self.marks}
            for i in ids:
                self.marks.pop(i, None)
            self.pending |= ids
        return ids

    def save_seen(self, ids: set[str]) -> None:
        """Write cleared marks to the file. A busy lock leaves them pending: the next update
        clears them (``pending``)."""
        if not ids:
            return
        with self._mu:
            keys = {self.keymap.get(i, i) for i in ids}

        def clear():
            data = self._read()
            if data is not None:
                for k in keys:
                    rec = data["agents"].get(k)
                    if rec:
                        rec["ev"] = None
                self._write(data)
            with self._mu:
                self.pending -= ids
            return True
        try:
            self._locked(clear)
        except OSError as exc:
            log.warning("unseen state not saved: %s", exc)
