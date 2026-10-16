"""The core <-> panel protocol (per-tab panels, docs/DESIGN.md "Per-tab panels"; the schema is
docs/protocol/panel-v1.md). Language-neutral on purpose: a Rust viewer can speak it.

Newline-delimited JSON over a Unix stream socket, one object per line, UTF-8. ``protocol`` is
"major.minor": the minor grows with additive fields, the major only on an incompatible change,
and a peer with another major is refused. Unknown fields are ignored.
"""

from __future__ import annotations

import dataclasses
import json
import os
import socket
import struct
import sys
from pathlib import Path

from .model import Agent, BgTask, Orchestrator, Phase, Swarm, Thread

PROTOCOL = "1.1"  # 1.1: state.panels, the open request
MAX_LINE = 4 * 1024 * 1024  # a longer line is refused and the connection closed


class TooLong(Exception):
    """A line over MAX_LINE: the peer is broken or hostile."""


def major(version: str | None) -> int | None:
    try:
        return int(str(version).split(".", 1)[0])
    except (TypeError, ValueError):
        return None


def compatible(version: str | None) -> bool:
    return major(version) == major(PROTOCOL)


def encode(msg: dict) -> bytes:
    line = json.dumps(msg, ensure_ascii=False, separators=(",", ":")).encode() + b"\n"
    if len(line) > MAX_LINE:
        raise TooLong(f"{len(line)} bytes")
    return line


class LineReader:
    """Bytes in, decoded messages out; a line longer than MAX_LINE raises TooLong. A line that is
    not a JSON object is skipped (an unknown message is not an error)."""

    def __init__(self) -> None:
        self._buf = b""

    def feed(self, data: bytes) -> list[dict]:
        *lines, self._buf = (self._buf + data).split(b"\n")  # one split per read: linear
        out = []
        for line in lines:
            if len(line) > MAX_LINE:
                raise TooLong(f"{len(line)} bytes")
            try:
                msg = json.loads(line)
            except (ValueError, UnicodeDecodeError):
                continue
            if isinstance(msg, dict):
                out.append(msg)
        if len(self._buf) > MAX_LINE:
            raise TooLong(f"{len(self._buf)} bytes without a newline")
        return out


def peer_cred(conn: socket.socket) -> tuple[int | None, int | None]:
    """The other end's (pid, uid): Linux SO_PEERCRED, macOS LOCAL_PEERPID and LOCAL_PEERCRED;
    (None, None) when the platform cannot say (and a uid check then fails closed)."""
    try:
        if hasattr(socket, "SO_PEERCRED"):  # Linux: struct ucred {pid, uid, gid}
            creds = conn.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, struct.calcsize("3i"))
            pid, uid, _ = struct.unpack("3i", creds)
            return pid, uid
        if sys.platform == "darwin":  # SOL_LOCAL 0: LOCAL_PEERCRED 1 (struct xucred), LOCAL_PEERPID 2
            uid = struct.unpack_from("2I", conn.getsockopt(0, 1, 76))[1]
            try:
                pid = struct.unpack("i", conn.getsockopt(0, 2, 4))[0]
            except OSError:
                pid = None
            return pid, uid
    except OSError:
        pass
    return None, None


# -- the model on the wire -----------------------------------------------------------------

def swarm_to_wire(swarm: Swarm) -> dict:
    """The panel's model, render-ready: the dataclasses' fields as plain JSON (model.py is the
    reference for their meaning)."""
    return {"generated_at": swarm.generated_at, "herdr_ok": swarm.herdr_ok, "inbox_ok": swarm.inbox_ok,
            "errors": list(swarm.errors), "stale_after": swarm.stale_after,
            "orchestrators": [dataclasses.asdict(o) for o in swarm.orchestrators]}


def _make(cls, d: dict, nested: dict | None = None):
    """``cls`` from ``d``, ignoring unknown fields (a newer minor) and building the nested lists."""
    names = {f.name for f in dataclasses.fields(cls)}
    kw = {k: v for k, v in d.items() if k in names}
    for key, sub in (nested or {}).items():
        if isinstance(kw.get(key), list):
            kw[key] = [sub(x) for x in kw[key] if isinstance(x, dict)]
    return cls(**kw)


def _agent(d: dict) -> Agent:
    return _make(Agent, d, {"bg": lambda x: _make(BgTask, x)})


def _thread(d: dict) -> Thread:
    return _make(Thread, d, {"phases": lambda x: _make(Phase, x), "agents": _agent})


def _orch(d: dict) -> Orchestrator:
    return _make(Orchestrator, d, {"threads": _thread, "bg": lambda x: _make(BgTask, x)})


def swarm_from_wire(d: dict) -> Swarm:
    sw = Swarm(herdr_ok=bool(d.get("herdr_ok", True)), inbox_ok=bool(d.get("inbox_ok", False)))
    sw.generated_at = float(d.get("generated_at") or 0.0)
    sw.errors = [str(e) for e in d.get("errors") or []]
    sw.stale_after = float(d.get("stale_after") or sw.stale_after)
    sw.orchestrators = [_orch(o) for o in d.get("orchestrators") or [] if isinstance(o, dict)]
    return sw


def state_message(swarm: Swarm, unseen: dict[str, str], focused_pane: str | None,
                  visible_tabs: list[str] | None, panels: dict[str, str] | None = None) -> dict:
    """The core's full snapshot for the panels (v1 is always a full snapshot, never a delta).
    ``visible_tabs`` None: unknown (a viewer then counts itself visible). ``panels``: tab ->
    its panel's pane (1.1), where a switch to that tab puts the keyboard."""
    return {"state": {"protocol": PROTOCOL, "swarm": swarm_to_wire(swarm), "unseen": dict(unseen),
                      "focused_pane": focused_pane, "visible_tabs": visible_tabs,
                      "panels": dict(panels or {})}}


# -- where the socket lives ----------------------------------------------------------------

SUN_PATH_MAX = 100  # sockaddr_un holds 108 bytes on Linux, 104 on macOS: stay under both


def socket_dir() -> Path:
    """The core's own directory for its socket and lock (the core makes it 0700 and refuses one
    that is not a private directory of ours, core.private_dir): under the plugin's state dir,
    or the runtime dir when that path is too long for a Unix socket."""
    from . import home
    name = "core-0123456789ab.sock"
    for d in (home.state_dir() / "sockets",
              Path(os.environ.get("XDG_RUNTIME_DIR") or "/tmp") / f"herdr-panorama-{os.getuid()}"):
        if len(str(d / name)) <= SUN_PATH_MAX:
            return d
    return Path("/tmp") / f"herdr-panorama-{os.getuid()}"  # macOS: long temp and state paths


def socket_path() -> Path:
    from . import home
    return socket_dir() / f"core-{home.session_key()}.sock"


def lock_path() -> Path:
    from . import home
    return socket_dir() / f"core-{home.session_key()}.lock"
