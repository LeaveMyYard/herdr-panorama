"""report_thread: merging into the explicit channel, and the MCP server over stdio."""

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


def test_tool_uses_the_session_from_the_environment(monkeypatch):
    monkeypatch.delenv("HERDR_PANE_ID", raising=False)
    monkeypatch.delenv("CLAUDE_CODE_SESSION_ID", raising=False)
    assert report_thread("t1")["status"] == "unavailable"
    monkeypatch.setenv("CLAUDE_CODE_SESSION_ID", "s2")
    out = report_thread("t1", title="Task 1\x1b[2J", phase="implement", next=["review"])
    assert out["status"] == "recorded" and out["phases"] == [("implement", "now"), ("review", "next")]
    assert events.current("s2", "t1").name == "Task 1[2J"  # control characters stripped
    assert report_thread("t1", status="bogus")["status"] == "error"


def test_tool_prefers_the_panes_current_session(fake_herdr, monkeypatch, tmp_path):
    snap = tmp_path / "snap.json"
    snap.write_text("{}")
    monkeypatch.setenv("HERDR_PANE_ID", "w1:p1")
    monkeypatch.setenv("CLAUDE_CODE_SESSION_ID", "stale")
    # the fake herdr answers `pane get` without a session: fall back to the environment
    assert report_thread("t2", phase="x")["status"] == "recorded"
    assert events.current("stale", "t2") is not None
    assert fake_herdr.calls_of("pane", "get") == [["pane", "get", "w1:p1"]]


async def test_mcp_server_over_stdio(tmp_path):
    from mcp import Client, StdioServerParameters
    env = dict(os.environ, CLAUDE_CODE_SESSION_ID="s3", PANORAMA_EVENTS_DIR=str(tmp_path / "ev"))
    env.pop("HERDR_PANE_ID", None)
    params = StdioServerParameters(command=sys.executable, args=["-m", "herdr_panorama.cli", "mcp"], env=env)
    async with Client(params, mode="legacy", read_timeout_seconds=60) as client:
        tools = {t.name: t for t in (await client.list_tools()).tools}
        assert set(tools) == {"report_thread"}
        res = await client.call_tool("report_thread", {"thread_id": "t", "title": "T", "phase": "build"})
    line = json.loads((tmp_path / "ev" / "s3.jsonl").read_text().splitlines()[-1])
    assert line["thread"] == "t" and line["phases"][0]["state"] == "now"
    assert not res.is_error



def test_m1_limits_and_no_wildcard_claims(monkeypatch):
    monkeypatch.setenv("CLAUDE_CODE_SESSION_ID", "s4")
    monkeypatch.delenv("HERDR_PANE_ID", raising=False)
    assert report_thread("t", agents=["*"])["status"] == "error"
    assert report_thread("t", agents=["ab*"])["status"] == "error"
    assert report_thread("t", agents=["Review PR*"])["status"] == "recorded"
    out = report_thread("t", phases_done=[f"p{i}" for i in range(100)], next=["n"] * 100)
    assert out["status"] == "error"
    out = report_thread("t", title="x\u202eevil\u2066", phase="a" * 500)
    d = events.current("s4", "t")
    assert d.name == "xevil" and len(d.phases[0]["name"]) <= 200


def test_m1_the_events_file_is_compacted(monkeypatch):
    monkeypatch.setenv("CLAUDE_CODE_SESSION_ID", "s5")
    monkeypatch.delenv("HERDR_PANE_ID", raising=False)
    for i in range(400):
        report_thread("t", title="Thread", phase=f"phase {i % 7}", next=["review", "merge"])
    path = events.events_path("s5")
    assert path.stat().st_size < 2 * events.COMPACT_AT
    d = events.current("s5", "t")
    assert d.name == "Thread" and d.phases[-1]["name"] == "merge"


def test_m1_short_patterns_never_match_on_read():
    assert not events.matches("*", "a1", "anything") and not events.matches("ab*", "a1", "abc")
    assert events.matches("Review*", "a1", "Review PR 7")
