"""Optional, read-only view of herdr-inbox's open questions.

Opens the inbox database with ``mode=ro`` and reads only columns of the
original 0.1 ``questions`` table, so every inbox version works. Anything wrong
(no file, locked, no table) means "no inbox": an empty result, never an error.
"""

from __future__ import annotations

import os
import sqlite3
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import quote


@dataclass
class Question:
    id: int
    session_id: str | None
    pane_id: str | None
    asked_by: str | None
    question: str
    created_at: str | None


def db_path() -> Path:
    env = os.environ.get("HERDR_INBOX_DB")
    if env:
        return Path(env)
    state = os.environ.get("XDG_STATE_HOME") or os.path.join(os.path.expanduser("~"), ".local", "state")
    return Path(state) / "herdr-inbox" / "inbox.db"


def open_questions(path: Path | None = None) -> list[Question] | None:
    """Open questions, or None when there is no usable inbox."""
    path = path or db_path()
    if not path.is_file():
        return None
    try:
        con = sqlite3.connect(f"file:{quote(str(path))}?mode=ro", uri=True, timeout=1.0)
    except sqlite3.Error:
        return None
    try:
        rows = con.execute(
            "SELECT id, session_id, pane_id, asked_by, question, created_at"
            " FROM questions WHERE status = 'open' ORDER BY id").fetchall()
    except sqlite3.Error:
        return None
    finally:
        con.close()
    return [Question(int(r[0]), r[1], r[2], r[3], str(r[4] or ""), r[5]) for r in rows]
