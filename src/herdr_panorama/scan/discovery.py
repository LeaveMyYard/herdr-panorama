"""Where Claude Code keeps a session's files.

Port of zoetrope ``src/provider/claude/discovery.rs`` (MIT): the project dir is
the cwd with every non-alphanumeric character replaced by ``-``; root
transcripts are ``<uuid>.jsonl``; a session's other files live under
``<uuid>/subagents/`` (``agent-<id>.jsonl`` + ``agent-<id>.meta.json``) and
``<uuid>/subagents/workflows/<runId>/`` (``journal.jsonl`` + agent files).
Missing directories are empty, never errors (Claude Code creates them lazily).

Added: a fallback search across all project dirs for a session id, because
Claude Code shortens very long project paths (zoetrope would miss those).
"""

from __future__ import annotations

import os
import re
from dataclasses import dataclass

_UUID = re.compile(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")


def projects_root() -> str:
    base = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.join(os.path.expanduser("~"), ".claude")
    return os.path.join(base, "projects")


def sanitize_cwd(cwd: str) -> str:
    return "".join(c if c.isascii() and c.isalnum() else "-" for c in cwd)


def is_uuid(stem: str) -> bool:
    """A Claude Code session id: a UUID (8-4-4-4-12 hex, any case), the whole string. The one
    validator: anything else (empty, ``..``, a separator, a glob, whitespace) never becomes a
    path, an rsync rule or a followed continuation."""
    return isinstance(stem, str) and bool(_UUID.fullmatch(stem))


def canonical_session_id(sid) -> str | None:
    """The canonical form of a session id (lowercase, as Claude Code names its files), or None
    when it is not one. Used wherever an id becomes a file name or is compared."""
    return sid.lower() if is_uuid(sid) else None


def find_root(session_id: str, cwd: str | None, root: str | None = None) -> str | None:
    """The ``<session>.jsonl`` for a session id: the cwd's project dir first, then every project
    dir. Only a UUID is looked up: an id comes from herdr or from a transcript (a remote
    machine's, too), and anything else (``..``, a separator, a glob) could name a file outside
    the transcripts folder."""
    sid = canonical_session_id(session_id)
    if sid is None:
        return None
    root = root or projects_root()
    name = f"{sid}.jsonl"
    if cwd:
        path = os.path.join(root, sanitize_cwd(cwd), name)
        if os.path.isfile(path):
            return path
    try:
        for entry in os.scandir(root):
            path = os.path.join(entry.path, name)
            if os.path.isfile(path):
                return path
    except OSError:
        pass
    return None


@dataclass(frozen=True)
class AgentFile:
    agent_id: str
    transcript: str
    meta: str
    workflow: str | None  # runId when under subagents/workflows/<runId>/


def subagents_dir(root_transcript: str) -> str:
    return os.path.join(root_transcript[: -len(".jsonl")], "subagents")


def scan_agent_files(directory: str, workflow: str | None = None) -> list[AgentFile]:
    out = []
    try:
        entries = list(os.scandir(directory))
    except OSError:
        return out
    for e in entries:
        name = e.name
        if name.startswith("agent-") and name.endswith(".jsonl") and len(name) > len("agent-.jsonl"):
            aid = name[len("agent-"):-len(".jsonl")]
            out.append(AgentFile(aid, e.path, os.path.join(directory, f"agent-{aid}.meta.json"), workflow))
    return sorted(out, key=lambda f: f.agent_id)


def scan_workflow_ids(subs: str) -> list[str]:
    try:
        return sorted(e.name for e in os.scandir(os.path.join(subs, "workflows")) if e.is_dir())
    except OSError:
        return []


def related(root_transcript: str) -> tuple[list[AgentFile], dict[str, str]]:
    """(agent files, {runId: journal path}) of one session."""
    subs = subagents_dir(root_transcript)
    files = scan_agent_files(subs)
    journals = {}
    for wf in scan_workflow_ids(subs):
        wdir = os.path.join(subs, "workflows", wf)
        journals[wf] = os.path.join(wdir, "journal.jsonl")
        files += scan_agent_files(wdir, wf)
    return files, journals


def workflow_summary_path(root_transcript: str, run_id: str) -> str:
    return os.path.join(root_transcript[: -len(".jsonl")], "workflows", f"{run_id}.json")
