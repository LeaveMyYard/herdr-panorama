"""Is a session's background Bash still running? (design turn 8, "waiting")

Claude Code runs a background Bash as a shell under its own process, with the command's output
going to ``<tmp>/claude-<uid>/<project>/<session>/tasks/<task id>.output``. While the shell
lives, it (or a child) holds that file open. So a task is alive when a descendant of the
session's Claude process has ``…/tasks/<task id>.output`` open, and dead when none does.

Read-only, Linux only (``/proc``): the Claude process's descendants come from
``/proc/<pid>/task/<tid>/children``, their open files from ``/proc/<pid>/fd``. Anything that
cannot be read is "unknown" (None), never "dead": the caller then falls back to the transcript
and its age cap. A shell left over from an earlier Claude process (the session was resumed) is
not a descendant of the current one, so it does not count: nobody will be told it finished.
"""

from __future__ import annotations

import os

PROC = "/proc"
MAX_PROCS = 4096  # a runaway tree never makes one collect slow


def _children(pid: int) -> list[int]:
    out: list[int] = []
    try:
        tids = os.listdir(f"{PROC}/{pid}/task")
    except OSError:
        return out
    for tid in tids:
        try:
            with open(f"{PROC}/{pid}/task/{tid}/children") as fh:
                out += [int(x) for x in fh.read().split()]
        except (OSError, ValueError):
            continue
    return out


def descendants(pid: int) -> list[int] | None:
    """Every process below ``pid``; None when ``pid`` cannot be read (gone, no procfs)."""
    if not os.path.isdir(f"{PROC}/{pid}/task"):
        return None
    seen: set[int] = set()
    stack = _children(pid)
    while stack and len(seen) < MAX_PROCS:
        p = stack.pop()
        if p in seen:
            continue
        seen.add(p)
        stack += _children(p)
    return sorted(seen)


def task_of(path: str) -> str | None:
    """``…/tasks/<id>.output`` -> ``<id>``."""
    head, name = os.path.split(path)
    if os.path.basename(head) == "tasks" and name.endswith(".output") and len(name) > 7:
        return name[:-7]
    return None


def live_tasks(pid: int) -> set[str] | None:
    """Task ids whose output file a descendant of ``pid`` holds open; None when unknown."""
    procs = descendants(pid)
    if procs is None:
        return None
    out: set[str] = set()
    for p in procs:
        try:
            fds = os.listdir(f"{PROC}/{p}/fd")
        except OSError:
            continue  # exited meanwhile, or not ours to read
        for fd in fds:
            try:
                target = os.readlink(f"{PROC}/{p}/fd/{fd}")
            except OSError:
                continue
            task = task_of(target)
            if task:
                out.add(task)
    return out


def start_token(pid: int) -> str | None:
    """The process start time (clock ticks since boot), so a reused pid is not mistaken."""
    try:
        with open(f"{PROC}/{pid}/stat") as fh:
            stat = fh.read()
        return stat[stat.rindex(")") + 2:].split()[19]
    except (OSError, ValueError, IndexError):
        return None
