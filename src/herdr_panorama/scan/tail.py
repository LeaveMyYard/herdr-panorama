"""Incremental reading of append-only files.

Port of zoetrope ``src/tailer/bytes.rs`` (MIT): stat, read the bytes appended
past the last offset, split on ``\\n``, return complete lines and buffer the
trailing partial line. A file that shrank or was replaced (``(dev, ino)``
changed) resets to offset 0. A partial line over 8 MB is dropped and reading
resyncs at the next newline. CRLF is tolerated and blank lines are dropped.
Lines are returned as bytes: callers decode only the lines they parse (a JSON
decode of invalid UTF-8 fails and the line is skipped, as in zoetrope).
"""

from __future__ import annotations

import os
from dataclasses import dataclass, field

MAX_PARTIAL = 8 * 1024 * 1024

MISSING, NO_CHANGE, RESET, LINES = "missing", "no_change", "reset", "lines"


@dataclass
class TailState:
    offset: int = 0
    partial: bytearray = field(default_factory=bytearray)
    overflowed: bool = False
    identity: tuple[int, int] | None = None
    mtime: float = 0.0
    size: int = 0


def read_appended(path: str, state: TailState) -> tuple[str, list[bytes]]:
    """(kind, lines). ``RESET`` means: forget everything derived from this file, then read again."""
    try:
        st = os.stat(path)
    except OSError:
        return MISSING, []
    identity = (st.st_dev, st.st_ino)
    replaced = state.identity is not None and identity != state.identity
    if st.st_size < state.offset or replaced:
        state.offset, state.overflowed, state.identity = 0, False, identity
        state.partial.clear()
        return RESET, []
    state.mtime, state.size = st.st_mtime, st.st_size
    if st.st_size == state.offset:
        state.identity = identity
        return NO_CHANGE, []
    state.identity = identity
    try:
        with open(path, "rb") as fh:
            fh.seek(state.offset)
            buf = fh.read(st.st_size - state.offset)
    except OSError:
        return MISSING, []
    state.offset += len(buf)
    return LINES, consume_bytes(state, buf)


def consume_bytes(state: TailState, appended: bytes) -> list[bytes]:
    """Pure over the byte stream: complete lines out, the trailing partial line buffered."""
    lines: list[bytes] = []
    start = 0
    while True:
        i = appended.find(b"\n", start)
        if i < 0:
            break
        if state.overflowed:
            state.overflowed = False
            state.partial.clear()
        elif state.partial:
            state.partial += appended[start:i]
            _push(lines, bytes(state.partial))
            state.partial.clear()
        else:
            _push(lines, appended[start:i])
        start = i + 1
    if not state.overflowed and start < len(appended):
        state.partial += appended[start:]
        if len(state.partial) > MAX_PARTIAL:
            state.partial.clear()
            state.overflowed = True
    return lines


def _push(lines: list[bytes], raw: bytes) -> None:
    if raw.endswith(b"\r"):
        raw = raw[:-1]
    if raw.strip():
        lines.append(raw)
