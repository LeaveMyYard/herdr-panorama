"""herdr-inbox integration: optional, read-only."""

import sqlite3

from herdr_panorama import inbox

from .scenario import inbox_db


def test_absent_inbox_is_none(tmp_path):
    assert inbox.open_questions(tmp_path / "missing.db") is None


def test_open_questions_only(tmp_path):
    db = inbox_db(tmp_path / "inbox.db", [
        {"id": 1, "session": "s1", "pane": "w1:p1", "question": "A?"},
        {"id": 2, "session": "s1", "pane": "w1:p1", "question": "B?", "status": "answered"},
    ])
    qs = inbox.open_questions(db)
    assert [(q.id, q.session_id, q.question) for q in qs] == [(1, "s1", "A?")]


def test_opened_read_only(tmp_path, monkeypatch):
    db = inbox_db(tmp_path / "inbox.db", [{"id": 1, "question": "A?"}])
    real_connect = sqlite3.connect
    seen = []

    def spy(target, *a, **kw):
        seen.append((target, kw.get("uri")))
        return real_connect(target, *a, **kw)

    monkeypatch.setattr(sqlite3, "connect", spy)
    inbox.open_questions(db)
    assert seen and seen[0][0].endswith("?mode=ro") and seen[0][1] is True


def test_unusable_database_is_none(tmp_path):
    bad = tmp_path / "inbox.db"
    bad.write_text("not a database")
    assert inbox.open_questions(bad) is None
    empty = tmp_path / "empty.db"
    sqlite3.connect(empty).close()
    assert inbox.open_questions(empty) is None  # no questions table


def test_env_override(tmp_path, monkeypatch):
    monkeypatch.setenv("HERDR_INBOX_DB", str(tmp_path / "x.db"))
    assert inbox.db_path() == tmp_path / "x.db"
