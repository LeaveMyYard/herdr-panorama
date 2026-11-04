"""report_thread, deprecated: listed but inert over stdio and in process; the explicit channel's
merge and compaction (still written by the frozen Python ``panorama emit``)."""

import json
import os
import sys

from herdr_panorama import events
from herdr_panorama.mcp_server import report_thread

T0 = 1_791_960_000.0


def iso(t):
    import time
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(t))


def test_report_merges_phases_and_keeps_times():
    events.report("s1", "pr-7", "PR #7", phase="author", next_phases=["review r1", "merge"], now=T0)
    events.report("s1", "pr-7", phase="review r1", phases_done=["author"], now=T0 + 600)
    d = events.current("s1", "pr-7")
    assert d.name == "PR #7" and d.status == "active"
    by = {p["name"]: p for p in d.phases}
    assert [p["name"] for p in d.phases] == ["author", "review r1", "merge"]
    assert by["author"]["state"] == "done" and by["author"]["start"] == iso(T0)
    assert by["author"]["end"] == iso(T0 + 600)
    assert by["review r1"]["state"] == "now" and by["merge"]["state"] == "next"
    events.report("s1", "pr-7", phase="merge", phases_done=["author", "review r1"], status="done", now=T0 + 900)
    d = events.current("s1", "pr-7")
    assert d.status == "done" and all(p["state"] == "done" for p in d.phases)
    assert {p["name"]: p["end"] for p in d.phases}["author"] == iso(T0 + 600)  # kept


IGNORED = {"status": "ignored", "reason": "threads were removed"}


def test_report_thread_records_nothing_and_says_ignored(monkeypatch):
    monkeypatch.delenv("HERDR_PANE_ID", raising=False)
    monkeypatch.delenv("CLAUDE_CODE_SESSION_ID", raising=False)
    assert report_thread("t1") == IGNORED
    monkeypatch.setenv("CLAUDE_CODE_SESSION_ID", "s2")
    assert report_thread("t1", title="Task 1", phase="implement", next=["review"]) == IGNORED
    assert events.current("s2", "t1") is None and not events.events_path("s2").exists()


def test_report_thread_never_asks_herdr_for_a_session(fake_herdr, monkeypatch):
    monkeypatch.setenv("HERDR_PANE_ID", "w1:p1")
    monkeypatch.setenv("CLAUDE_CODE_SESSION_ID", "stale")
    assert report_thread("t2", phase="x") == IGNORED
    assert fake_herdr.calls_of("pane", "get") == []
    assert not events.events_path("stale").exists()


async def test_mcp_server_over_stdio(tmp_path):
    from mcp import Client, StdioServerParameters
    env = dict(os.environ, CLAUDE_CODE_SESSION_ID="s3", PANORAMA_EVENTS_DIR=str(tmp_path / "ev"))
    env.pop("HERDR_PANE_ID", None)
    params = StdioServerParameters(command=sys.executable, args=["-m", "herdr_panorama.cli", "mcp"], env=env)
    async with Client(params, mode="legacy", read_timeout_seconds=60) as client:
        tools = {t.name: t for t in (await client.list_tools()).tools}
        assert set(tools) == {"report_thread"}  # still listed: no "Unknown tool" for old sessions
        assert tools["report_thread"].description.startswith("Deprecated: has no effect.")
        assert tools["report_thread"].input_schema["required"] == ["thread_id"]
        res = await client.call_tool("report_thread", {"thread_id": "t", "title": "T", "phase": "build"})
    assert not res.is_error and res.structured_content == IGNORED
    assert not (tmp_path / "ev").exists()


def test_any_arguments_are_ignored(monkeypatch):
    monkeypatch.setenv("CLAUDE_CODE_SESSION_ID", "s4")
    monkeypatch.delenv("HERDR_PANE_ID", raising=False)
    for kwargs in ({"agents": ["*"]}, {"status": "bogus"}, {"next": ["n"] * 100},
                   {"title": "x\u202eevil\u2066", "phase": "a" * 500}):
        assert report_thread("t", **kwargs) == IGNORED
    assert not events.events_path("s4").exists()


def test_m1_the_events_file_is_compacted():
    for i in range(400):
        events.report("s5", "t", "Thread", phase=f"phase {i % 7}", next_phases=["review", "merge"], now=T0 + i)
    path = events.events_path("s5")
    assert path.stat().st_size < 2 * events.COMPACT_AT
    d = events.current("s5", "t")
    assert d.name == "Thread" and d.phases[-1]["name"] == "merge"


def test_m1_short_patterns_never_match_on_read():
    assert not events.matches("*", "a1", "anything") and not events.matches("ab*", "a1", "abc")
    assert events.matches("Review*", "a1", "Review PR 7")
