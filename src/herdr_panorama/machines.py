"""Agents on herdr's saved SSH machines, next to the local ones. See docs/REMOTE.md.

Per machine, a background thread (``Watcher``) runs on its own cadence and never blocks the
view:

1. herdr state through herdr's own forwarding: ``herdr --machine <id> api snapshot``
   (read-only; forwarding never starts a remote server).
2. the Claude transcripts of the sessions that snapshot names, mirrored with ``rsync``
   over ssh into the plugin state dir (append-only, so each refresh copies only new bytes).
   ssh runs in batch mode (never prompts), with a connect timeout and a shared control
   connection.
3. the same collector as local sessions, pointed at that mirror; then every orchestrator is
   tagged with its machine and its herdr ids are namespaced (``<label>:w1:p2``), because ids
   are per server.

The view merges the latest result of every machine with the local swarm. A machine that
fails keeps its last result, marked stale; one that never answered shows as unreachable.
"""

from __future__ import annotations

import json
import logging
import os
import re
import shutil
import signal
import subprocess
import threading
import time
from dataclasses import dataclass, field
from pathlib import Path

from . import config, herdr
from .collect import Collector
from .model import Swarm

log = logging.getLogger("herdr_panorama.machines")

REMOTE_SECONDS = 5.0  # a machine's refresh cadence (its own thread)
SNAPSHOT_TIMEOUT = 8.0
SYNC_TIMEOUT = 30.0
STALE_SECONDS = 30.0  # a result older than this is shown as stale
MAX_FILE = "64M"  # rsync --max-size: no single mirrored file is larger
_SESSION = re.compile(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")
_locks: dict[str, threading.Lock] = {}
_locks_guard = threading.Lock()


def valid_session(sid: str) -> bool:
    """A Claude Code session id: a UUID. Anything else never becomes an rsync rule or a
    path to delete (it could be empty, ``..`` or carry a separator)."""
    return bool(_SESSION.match(sid or ""))


def machine_lock(machine_id: str) -> threading.Lock:
    """One machine's mirror is written by one sync at a time (mirror + prune)."""
    with _locks_guard:
        return _locks.setdefault(machine_id, threading.Lock())


@dataclass
class Machine:
    id: str
    label: str
    target: str  # the ssh target of the saved profile


def selection() -> str | list[str]:
    """``machines`` in config.toml: "all" (default), "none", or a list of labels."""
    value = config.load().get("machines", "all")
    if isinstance(value, list):
        return [str(v) for v in value]
    v = str(value).strip().lower()
    return "none" if v in ("none", "off", "false", "0", "") else "all"


def saved() -> list[Machine]:
    """herdr's enabled saved machines (``herdr machine list --json``), filtered by config."""
    sel = selection()
    if sel == "none":
        return []
    out, _ = herdr.run_raw("machine", "list", "--json")
    try:
        rows = json.loads(out or "[]")
    except ValueError:
        return []
    found = []
    for r in rows:
        if not (isinstance(r, dict) and r.get("enabled") and r.get("id") and r.get("target")):
            continue
        m = Machine(str(r["id"]), str(r.get("label") or r["id"]), str(r["target"]))
        if not safe_target(m.target):
            log.warning("machine %s: ssh target %r is not used (it would read as an option)", m.label, m.target)
            continue
        found.append(m)
    return found if sel == "all" else [m for m in found if m.label in sel]


def safe_target(target: str) -> bool:
    """An ssh target that can never become an rsync or ssh option."""
    return bool(target) and not target.startswith("-") and not any(c.isspace() for c in target)


def prefix(machine: Machine) -> str:
    """The namespace of this machine's herdr ids: its stable id (labels are for display and
    can repeat or contain ``:``)."""
    return f"{machine.id}:"


def local_id(pane_id: str, machine_id: str) -> str:
    """A namespaced pane id back to the id on its own server."""
    p = f"{machine_id}:"
    return pane_id[len(p):] if pane_id.startswith(p) else pane_id


def mirror_root(machine: Machine) -> Path:
    from .home import state_dir
    return state_dir() / "remote" / "".join(c for c in machine.id if c.isalnum()) / "projects"


def _ssh_options() -> list[str]:
    run = os.environ.get("XDG_RUNTIME_DIR") or f"/run/user/{os.getuid()}"
    return ["-o", "BatchMode=yes", "-o", "ConnectTimeout=5", "-o", "ServerAliveInterval=5",
            "-o", "ControlMaster=auto", "-o", f"ControlPath={run}/panorama-ssh-%C", "-o", "ControlPersist=60"]


def remote_snapshot(machine: Machine) -> tuple[dict | None, str | None]:
    out, err = herdr.run_raw("--machine", machine.id, "api", "snapshot", timeout=SNAPSHOT_TIMEOUT)
    if out is None:
        return None, err
    try:
        snap = (json.loads(out).get("result") or {}).get("snapshot")
    except (ValueError, AttributeError):
        return None, "unreadable snapshot"
    return (snap, None) if isinstance(snap, dict) else (None, "no snapshot")


def session_ids(snapshot: dict) -> list[str]:
    ids = []
    for a in snapshot.get("agents") or []:
        sess = a.get("agent_session") if isinstance(a, dict) else None
        if isinstance(sess, dict) and sess.get("kind", "id") == "id" and sess.get("value"):
            ids.append(str(sess["value"]))
    return ids


def _rules(sessions: list[str], kind: str) -> list[str]:
    """rsync filter rules for exactly the files the scanner reads of these sessions:
    ``jsonl`` (append-only: the transcript, agent transcripts, workflow journals) or ``json``
    (rewritten in place: agent metas, workflow run summaries)."""
    rules = ["--include=*/"]
    for sid in sessions:
        if not valid_session(sid):
            continue
        if kind == "jsonl":
            rules += [f"--include=*/{sid}.jsonl", f"--include=*/{sid}/subagents/agent-*.jsonl",
                      f"--include=*/{sid}/subagents/workflows/*/journal.jsonl",
                      f"--include=*/{sid}/subagents/workflows/*/agent-*.jsonl"]
        else:
            rules += [f"--include=*/{sid}/subagents/agent-*.meta.json",
                      f"--include=*/{sid}/subagents/workflows/*/agent-*.meta.json",
                      f"--include=*/{sid}/workflows/*.json"]
    return rules + ["--exclude=*"]


def _run(argv: list[str], timeout: float) -> str | None:
    """Run in its own process group; on timeout the whole group goes (rsync and its ssh)."""
    try:
        p = subprocess.Popen(argv, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
                             start_new_session=True)
    except OSError as exc:
        return f"rsync: {exc}"
    try:
        _, err = p.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(p.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        p.communicate()
        return "rsync timed out"
    return None if p.returncode in (0, 24) else (err or f"rsync exit {p.returncode}").strip()[:300]


_append_ok: dict = {}


def append_supported() -> bool:
    """Whether this rsync knows ``--append-verify`` (rsync 3.x does; macOS's openrsync does
    not). Without it the .jsonl pass uses rsync's normal delta transfer, which also sends
    little more than the appended bytes."""
    if "ok" not in _append_ok:
        try:
            out = subprocess.run(["rsync", "--help"], capture_output=True, text=True, timeout=5)
            _append_ok["ok"] = "--append-verify" in (out.stdout + out.stderr)
        except (OSError, subprocess.SubprocessError):
            _append_ok["ok"] = False
    return _append_ok["ok"]


def mirror(source: str, dest: Path, sessions: list[str], ssh: list[str] | None = None,
           timeout: float = SYNC_TIMEOUT) -> str | None:
    """Two passes into ``dest``: the ``.jsonl`` files append-only (``--append-verify``: only
    new bytes travel), then the ``.json`` files normally (they are rewritten in place, maybe
    shorter or at the same size). No file over MAX_FILE. Error or None."""
    base = ["rsync", "-rt", "--prune-empty-dirs", f"--max-size={MAX_FILE}"]
    if ssh:
        base += ["-e", " ".join(ssh)]
    append = ["--append-verify"] if append_supported() else []
    for kind, extra in (("jsonl", append), ("json", [])):
        err = _run([*base, *extra, *_rules(sessions, kind), source, f"{dest}/"], timeout)
        if err:
            return err
    return None


def prune(dest: Path, sessions: list[str], keep_seconds: float, now: float | None = None) -> list[str]:
    """Drop mirrored sessions the machine has not named for longer than ``keep_seconds``
    (the done window). ``seen.json`` in the mirror keeps when each was last named."""
    now = time.time() if now is None else now
    path = dest / "seen.json"
    try:
        seen = json.loads(path.read_text())
        seen = seen if isinstance(seen, dict) else {}
    except (OSError, ValueError):
        seen = {}
    for sid in sessions:
        if valid_session(sid):
            seen[sid] = now
    seen = {k: v for k, v in seen.items() if valid_session(k) and isinstance(v, (int, float))}
    gone = []
    for proj in (p for p in dest.iterdir() if p.is_dir() and not p.is_symlink()) if dest.exists() else []:
        base = proj.resolve()
        for f in proj.glob("*.jsonl"):
            sid = f.stem
            if not valid_session(sid) or f.is_symlink():
                continue  # not a session's transcript: never a reason to delete anything
            if sid in seen and now - seen[sid] <= keep_seconds:
                continue
            if sid not in seen:
                seen[sid] = now  # mirrored before this bookkeeping: start its clock now
                continue
            d = proj / sid
            if d.is_symlink() or (d.exists() and d.resolve().parent != base):
                continue  # only a real directory directly inside this project dir
            f.unlink(missing_ok=True)
            if d.is_dir():
                shutil.rmtree(d, ignore_errors=True)
            seen.pop(sid, None)
            gone.append(sid)
    try:
        path.write_text(json.dumps(seen))
    except OSError:
        pass
    return gone


def sync(machine: Machine, sessions: list[str]) -> str | None:
    """Mirror what the scanner reads of these sessions from the machine's
    ``~/.claude/projects``, then prune sessions gone longer than the done window."""
    dest = mirror_root(machine)
    dest.mkdir(parents=True, exist_ok=True)
    sessions = [sid for sid in sessions if valid_session(sid)]
    with machine_lock(machine.id):  # never two syncs into one mirror (duplicate appends)
        err = None
        if sessions:
            if not safe_target(machine.target):
                return "unsafe ssh target"
            err = mirror(f"{machine.target}:.claude/projects/", dest, sessions, ["ssh", *_ssh_options()])
        prune(dest, sessions, config.done_window())
    return err


def namespace(swarm: Swarm, machine: Machine) -> Swarm:
    """Ids are per server: every orchestrator gets its machine (``machine_id`` for identity
    and commands, ``machine`` the label for display) and ``<machine id>:`` ids."""
    for o in swarm.orchestrators:
        o.machine, o.machine_id = machine.label, machine.id
        o.pane_id = prefix(machine) + o.pane_id
        o.tab_id = prefix(machine) + o.tab_id if o.tab_id else o.tab_id
        o.inbox, o.inbox_texts = [], []  # the inbox is local; remote questions come later
    swarm.docked = None  # panorama never docks into a remote server
    return swarm


@dataclass
class Result:
    swarm: Swarm | None = None
    at: float = 0.0  # monotonic time of the last good result
    error: str | None = None
    tried: bool = False


class Watcher:
    """One machine's background refresh. ``result`` is read by the view thread."""

    def __init__(self, machine: Machine, now=time.time):
        self.machine = machine
        self.collector = Collector(root=str(mirror_root(machine)), now=now, procs=False)  # transcripts only
        self.result = Result()
        self._stop = threading.Event()

    def refresh(self) -> None:
        snap, err = remote_snapshot(self.machine)
        if snap is None:
            self.result = Result(self.result.swarm, self.result.at, err or "unreachable", True)
            return
        serr = sync(self.machine, session_ids(snap))
        if serr:
            log.warning("%s: transcripts: %s", self.machine.label, serr)
        swarm = namespace(self.collector.collect(snap, None), self.machine)
        if serr:
            swarm.errors.append(f"{self.machine.label}: transcripts not synced: {serr}"[:200])
        self.result = Result(swarm, time.monotonic(), None, True)

    def run(self) -> None:
        while not self._stop.is_set():
            try:
                self.refresh()
            except Exception as exc:  # noqa: BLE001 - one machine never takes the view down
                self.result = Result(self.result.swarm, self.result.at, f"{type(exc).__name__}: {exc}", True)
            self._stop.wait(REMOTE_SECONDS)

    def start(self) -> None:
        self._thread = threading.Thread(target=self.run, daemon=True, name=f"machine-{self.machine.label}")
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()

    def join(self, timeout: float | None = None) -> None:
        t = getattr(self, "_thread", None)
        if t is not None:
            t.join(timeout)


@dataclass
class Remotes:
    """Every machine's watcher; ``merge`` adds their latest results to a local swarm."""

    watchers: list[Watcher] = field(default_factory=list)
    chosen: object = None  # the ``machines`` setting these watchers were started for

    @classmethod
    def start(cls) -> "Remotes":
        r = cls([Watcher(m) for m in saved()], selection())
        for w in r.watchers:
            w.start()
        return r

    def follow_config(self) -> None:
        """A changed ``machines`` setting applies without a restart: the config file is
        re-read only when its mtime changes, and the watchers are replaced only then."""
        sel = selection()
        if sel == self.chosen:
            return
        self.chosen = sel
        wanted = {m.id: m for m in saved()}
        kept = []
        for w in self.watchers:
            if w.machine.id in wanted and w.machine.target == wanted[w.machine.id].target:
                kept.append(w)  # still selected: keeps running, never restarted
                del wanted[w.machine.id]
            else:
                w.stop()  # removed; a sync it has under way finishes under the machine's lock
        added = [Watcher(m) for m in wanted.values()]
        self.watchers = kept + added
        for w in added:
            w.start()

    def merge(self, local: Swarm) -> Swarm:
        self.follow_config()
        now = time.monotonic()
        for w in self.watchers:
            res = w.result
            label = w.machine.label
            if res.swarm is not None:
                local.orchestrators += res.swarm.orchestrators
                local.errors += res.swarm.errors
            if res.tried and res.swarm is None:
                local.errors.append(f"{label}: unreachable ({res.error})"[:200])
            elif res.swarm is not None and (res.error or now - res.at > STALE_SECONDS):
                local.errors.append(f"{label}: stale ({res.error or 'no answer'})"[:200])
        return local

    def stop(self) -> None:
        for w in self.watchers:
            w.stop()
