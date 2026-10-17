"""Opt-in Claude Code hooks (the Claude Code plugin in .claude-plugin/).

Run directly by ``hooks/hooks.json`` as ``sh -c 'sh "$0" "$1" <kind> …; exit 0' run-hook.sh hooks.py``
(``hooks/run-hook.sh`` execs ``python3 -I hooks.py <kind>`` with the interpreter cached in ``$CLAUDE_PLUGIN_DATA``):
stdlib only, no uv and no virtualenv on the hook path, and the shell forces exit 0.
For SubagentStop, exit code 2 "prevents the subagent from stopping" and its stderr
is fed back to the agent, so nothing on this path may ever exit 2 (review B1).

``SubagentStart`` / ``SubagentStop`` (matcher ``.+``: Claude Code's internal agents have an
empty ``agent_type`` and are skipped) append one line per event to
``${PANORAMA_HOOKS_DIR:-${XDG_STATE_HOME:-~/.local/state}/herdr-panorama/hooks}/<session_id>.jsonl``:

    {"v": 1, "event": "start"|"stop", "agent": "<id>", "agent_type": "...", "ts": <epoch>,
     "pid": <claude pid>, "pstart": "<process start token>", "pidns": "<pid namespace>"}

``SessionStart`` appends ``{"event": "session", ...}`` with the same process identity
(a resume shows up as a new process), prunes hook files older than 14 days, and
prints the short ``report_thread`` instruction once per session, inside herdr only.

Payload fields (official hooks reference, https://code.claude.com/docs/en/hooks):
``session_id``, ``agent_id``, ``agent_type``; SubagentStop also ``stop_hook_active``
(true: Claude Code is already continuing because of a stop hook; nothing is recorded).

Process identity is recorded only when the Claude process is recognised (a native
install's ``claude`` binary or versioned binary, or node running the claude-code
package); otherwise no pid is written and the scanner never calls an agent ``lost``.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from pathlib import Path

VERSION = 1
MAX_DEPTH = 32
PRUNE_AFTER = 14 * 86400

SESSION_TEXT = (
    "herdr-panorama shows your work in a swarm view. When you run multi-step work (a PR through "
    "review rounds, a plan's tasks, several subagents on one goal), call report_thread when a thread "
    "starts and whenever its phase changes: thread_id, title, the current phase, finished phases, "
    "next phases. One call per change; skip it for single-step tasks."
)


def hooks_dir() -> Path:
    env = os.environ.get("PANORAMA_HOOKS_DIR")
    if env:
        return Path(env)
    state = os.environ.get("XDG_STATE_HOME") or os.path.join(os.path.expanduser("~"), ".local", "state")
    return Path(state) / "herdr-panorama" / "hooks"


def hooks_path(session_id: str) -> Path:
    safe = "".join(c for c in session_id if c.isalnum() or c in "-_")
    return hooks_dir() / f"{safe}.jsonl"


def normalize_agent_id(value: str) -> str:
    """The docs show both ``agent-abc123`` and ``def456``; files are ``agent-<id>.jsonl``."""
    return value[len("agent-"):] if value.startswith("agent-") else value


# -- process identity ----------------------------------------------------------------

def _stat(pid: int) -> tuple[int, str, str] | None:
    """(parent pid, command name, start token) of ``pid``."""
    try:
        with open(f"/proc/{pid}/stat") as fh:
            stat = fh.read()
        comm = stat[stat.index("(") + 1: stat.rindex(")")]
        fields = stat[stat.rindex(")") + 2:].split()
        return int(fields[1]), comm, fields[19]  # ppid, comm, starttime (clock ticks since boot)
    except FileNotFoundError:
        pass  # no procfs (macOS)
    except (OSError, ValueError, IndexError):
        return None
    try:
        out = subprocess.run(["ps", "-o", "ppid=,lstart=,comm=", "-p", str(pid)], capture_output=True, text=True,
                             timeout=1).stdout.split()
        return int(out[0]), os.path.basename(out[-1]), " ".join(out[1:-1])
    except (OSError, subprocess.SubprocessError, ValueError, IndexError):
        return None


def _cmdline(pid: int) -> list[str]:
    try:
        with open(f"/proc/{pid}/cmdline", "rb") as fh:
            return [a.decode("utf-8", "replace") for a in fh.read().split(b"\0") if a]
    except FileNotFoundError:
        pass
    except OSError:
        return []
    try:
        return subprocess.run(["ps", "-o", "command=", "-p", str(pid)], capture_output=True, text=True,
                              timeout=1).stdout.split()
    except (OSError, subprocess.SubprocessError):
        return []


CLAUDE_ENTRY = ("/@anthropic-ai/claude-code/cli.js", "/@anthropic-ai/claude-code/cli.mjs")


def is_claude(comm: str, argv: list[str]) -> bool:
    """A native install (``claude``, or ``~/.local/share/claude/versions/<v>``) or node
    running the ``@anthropic-ai/claude-code`` entry script (``.../cli.js`` or ``cli.mjs``). A bare
    package name is not Claude: ``npx @anthropic-ai/claude-code`` is the launcher above it."""
    if comm == "claude":
        return True
    first = argv[0] if argv else ""
    if os.path.basename(first) == "claude" or "/claude/versions/" in first:
        return True
    if os.path.basename(first).startswith("node"):
        return any(a.endswith(CLAUDE_ENTRY) for a in argv[1:3])
    return False


def claude_process() -> tuple[int, str] | None:
    """(pid, start token) of the nearest Claude process above this one, or None."""
    pid = os.getppid()
    for _ in range(MAX_DEPTH):
        if pid <= 1:
            return None
        info = _stat(pid)
        if info is None:
            return None
        ppid, comm, start = info
        if is_claude(comm, _cmdline(pid)):
            return pid, start
        pid = ppid
    return None


def own_pidns() -> str | None:
    try:
        return os.readlink("/proc/self/ns/pid")
    except OSError:
        return None


def process_alive(pid: int, start: str, pidns: str | None = None) -> bool | None:
    """True/False, or None when it cannot be told (then nothing is called lost): another pid
    namespace than ours, no permission, no process table."""
    if pidns is not None and pidns != own_pidns():
        return None
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        pass
    except OSError:
        return None
    info = _stat(pid)
    if info is None:
        return None
    return info[2] == start  # a reused pid has another start time


# -- the hook ------------------------------------------------------------------------

def _append(session: str, rec: dict) -> Path:
    proc = claude_process()
    if proc:
        rec["pid"], rec["pstart"] = proc
        rec["pidns"] = own_pidns()
    path = hooks_path(session)
    path.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o644)
    try:
        os.write(fd, (json.dumps(rec) + "\n").encode())  # one write: O_APPEND keeps lines whole
    finally:
        os.close(fd)
    return path


def record(event: str, payload: dict) -> None:
    session, agent = payload.get("session_id"), payload.get("agent_id")
    if not isinstance(session, str) or not session or not isinstance(agent, str) or not agent:
        return
    if payload.get("stop_hook_active") is True:
        return  # Claude Code is already continuing because of a stop hook: stay out of it
    _append(session, {"v": VERSION, "event": event, "agent": normalize_agent_id(agent),
                      "agent_type": payload.get("agent_type") if isinstance(payload.get("agent_type"), str) else None,
                      "ts": time.time()})


def _told(path: Path) -> bool:
    try:
        with open(path, "rb") as fh:
            return any(b'"event": "session"' in line and b'"told": true' in line for line in fh)
    except OSError:
        return False


def prune(now: float | None = None) -> None:
    now = time.time() if now is None else now
    try:
        entries = list(os.scandir(hooks_dir()))[:500]
    except OSError:
        return
    for e in entries:
        try:
            if e.name.endswith(".jsonl") and e.stat().st_mtime < now - PRUNE_AFTER:
                os.unlink(e.path)
        except OSError:
            pass


def session_start(payload: dict) -> str:
    """Record the session's process (resumes show up as a new one); return the instruction
    the first time a session starts inside herdr, else nothing."""
    prune()
    session = payload.get("session_id")
    in_herdr = bool(os.environ.get("HERDR_PANE_ID") or os.environ.get("HERDR_ENV"))
    if not isinstance(session, str) or not session:
        return ""
    tell = in_herdr and not _told(hooks_path(session))
    _append(session, {"v": VERSION, "event": "session", "agent": "", "source": payload.get("source"),
                      "ts": time.time(), "told": tell})
    if not tell:
        return ""
    return json.dumps({"hookSpecificOutput": {"hookEventName": "SessionStart", "additionalContext": SESSION_TEXT}})


def main(kind: str, stdin=None) -> int:
    """Entry point (``python3 -I hooks.py <kind>`` or ``panorama hook <kind>``). Always 0."""
    try:
        raw = (stdin or sys.stdin).read()
        payload = json.loads(raw) if raw.strip() else {}
        if not isinstance(payload, dict):
            payload = {}
        if kind == "subagent-start":
            record("start", payload)
        elif kind == "subagent-stop":
            record("stop", payload)
        elif kind == "session-start":
            out = session_start(payload)
            if out:
                sys.stdout.write(out)
    except Exception:  # noqa: BLE001 - a hook never fails the session
        pass
    return 0


if __name__ == "__main__":
    try:
        main(sys.argv[1] if len(sys.argv) > 1 else "")
        sys.stdout.flush()
    finally:
        os._exit(0)  # whatever happened, exit 0 (hooks.json's `exit 0` is the second guard)
