"""End to end over the demo scenario: snapshot + transcripts + workflow + explicit channel + inbox."""

import json

import pytest

from herdr_panorama.collect import Collector
from herdr_panorama.inbox import open_questions
from herdr_panorama.model import CXL, DONE, FAIL, INF, REV, WAIT

from .scenario import NOW, build_demo


def collect(tmp_path, monkeypatch, now=NOW, window=6 * 3600):
    d = build_demo(tmp_path)
    monkeypatch.setenv("PANORAMA_EVENTS_DIR", str(d["events"]))
    c = Collector(root=str(d["projects"]), now=lambda: now, done_window=window)
    return c, d, c.collect(d["snapshot"], open_questions(d["inbox"]))


def test_demo_swarm_shape(tmp_path, monkeypatch):
    _, _, sw = collect(tmp_path, monkeypatch)
    by = {o.tab_label: o for o in sw.orchestrators}
    assert list(by) == ["Billing API", "Search refactor", "Docs site", "Release notes"]  # the shell pane is skipped
    billing = by["Billing API"]
    assert billing.inbox == [1, 2] and billing.flavour == "pr-review"
    threads = {t.name: t for t in billing.threads}
    t42 = threads["#42"]
    status = {a.id[:8]: a.status for a in t42.agents}
    assert status == {"a1author": DONE, "a4review": REV, "a2review": DONE, "a3explor": DONE}
    nested = next(a for a in t42.agents if a.id.startswith("a3"))
    assert nested.parent == "a1author0000000001" and nested.depth == 2
    assert [p.name for p in t42.phases] == ["author", "review r1", "author 2", "review r2"]
    a1 = next(a for a in t42.agents if a.id.startswith("a1author"))
    assert a1.resumes == 1 and a1.tool_calls == 44  # the notice's count wins over the file's
    assert threads["#44"].status == "failed" and any(a.status == FAIL for a in threads["#44"].agents)
    assert [a.status for a in threads["Draft retry banner copy"].agents] == [WAIT]
    assert threads["Billing API"].source == "session"  # Q1 is not attributed to any agent
    wf = by["Search refactor"].threads[0]
    assert wf.source == "workflow" and wf.name == "search-index-rebuild"
    assert [(p.name, p.state) for p in wf.phases] == [("Plan", "done"), ("Build", "now")]
    docs = by["Docs site"]
    assert docs.pending == ["Publish the v2 docs now or after the release?"]
    explicit = next(t for t in docs.threads if t.source == "explicit")
    assert explicit.name == "Docs v2 release" and explicit.agents[0].role == "writer" and explicit.pct == 60
    assert by["Release notes"].threads[0].status == "done"


def test_done_window_hides_old_threads(tmp_path, monkeypatch):
    _, _, sw = collect(tmp_path, monkeypatch, window=60)
    notes = next(o for o in sw.orchestrators if o.tab_label == "Release notes")
    assert notes.threads == [] and notes.hidden_done == 0  # its agent is outside the window entirely


def test_later_ticks_read_only_what_was_appended(tmp_path, monkeypatch):
    c, d, _ = collect(tmp_path, monkeypatch)
    sess = next(iter(c.sessions.values()))
    offsets = {p: f.tail.offset for p, f in sess.files.items()}
    sw = c.collect(d["snapshot"], open_questions(d["inbox"]))
    assert {p: f.tail.offset for p, f in sess.files.items()} == offsets
    assert sw.orchestrators[0].tab_label == "Billing API"


def test_lost_agents_end_silently(tmp_path, monkeypatch):
    _, _, sw = collect(tmp_path, monkeypatch, now=NOW + 7 * 3600, window=24 * 3600)
    ags = [a for o in sw.orchestrators for a in o.agents() if not a.id.startswith("main:")]
    assert all(a.status in (DONE, CXL, FAIL, INF) for a in ags)
    assert any(a.status == INF and a.inferred for a in ags)  # issue #2: never a plain done


def test_inbox_tab_prefix_is_stripped(tmp_path, monkeypatch):
    d = build_demo(tmp_path)
    d["snapshot"]["tabs"][0]["label"] = "❓2 Billing API"
    monkeypatch.setenv("PANORAMA_EVENTS_DIR", str(d["events"]))
    sw = Collector(root=str(d["projects"]), now=lambda: NOW, done_window=6 * 3600).collect(d["snapshot"], None)
    assert sw.orchestrators[0].tab_label == "Billing API"


def test_a_session_continued_in_a_new_transcript_is_followed(tmp_path, monkeypatch):
    """The operator's report: Claude Code moved the conversation to a new session id (the pane's
    claude process kept running) while herdr still reported the old one; panorama read the old,
    finished transcript and showed 0 agents. The old transcript's last record names the new
    session (shape captured from Claude Code 2.1.301), and the collector follows it."""
    import os
    from herdr_panorama.scan import discovery
    from .scenario import S1
    _, _, base = collect(tmp_path / "base", monkeypatch)
    want = next(o for o in base.orchestrators if o.tab_label == "Billing API")

    d = build_demo(tmp_path / "moved")
    monkeypatch.setenv("PANORAMA_EVENTS_DIR", str(d["events"]))
    new = "55555555-5555-4555-8555-555555555555"
    old_root = discovery.find_root(S1, None, str(d["projects"]))
    new_root = os.path.join(os.path.dirname(old_root), f"{new}.jsonl")
    os.rename(old_root, new_root)
    if os.path.isdir(old_root[:-len(".jsonl")]):
        os.rename(old_root[:-len(".jsonl")], new_root[:-len(".jsonl")])
    with open(old_root, "w") as fh:  # what is left of the old session: its end, pointing on
        fh.write(f'{{"type":"ai-title","aiTitle":"Billing API","sessionId":"{S1}"}}\n')
        fh.write(f'{{"type":"continued-in","timestamp":"2026-10-16T18:41:47.545Z","sessionId":"{S1}",'
                 f'"continuedInSessionId":"{new}"}}\n')  # byte for byte as Claude Code writes it
    c = Collector(root=str(d["projects"]), now=lambda: NOW, done_window=6 * 3600)
    sw = c.collect(d["snapshot"], open_questions(d["inbox"]))  # herdr still says S1
    got = next(o for o in sw.orchestrators if o.tab_label == "Billing API")
    assert got.session_id == new and got.transcript == new_root
    assert [t.name for t in got.threads] == [t.name for t in want.threads]
    assert len(list(got.agents())) == len(list(want.agents())) > 1
    assert got.inbox == [1, 2]  # questions asked under the old id still belong to it


def test_a_continuation_loop_is_not_followed_forever(tmp_path, monkeypatch):
    import os
    from herdr_panorama.scan import discovery
    from .scenario import S1
    d = build_demo(tmp_path)
    monkeypatch.setenv("PANORAMA_EVENTS_DIR", str(d["events"]))
    root = discovery.find_root(S1, None, str(d["projects"]))
    other = "66666666-6666-4666-8666-666666666666"
    with open(root, "a") as fh:
        fh.write(f'{{"type":"continued-in","sessionId":"{S1}","continuedInSessionId":"{other}"}}\n')
    with open(os.path.join(os.path.dirname(root), f"{other}.jsonl"), "w") as fh:
        fh.write(f'{{"type":"continued-in","sessionId":"{other}","continuedInSessionId":"{S1}"}}\n')
    c = Collector(root=str(d["projects"]), now=lambda: NOW, done_window=6 * 3600)
    sw = c.collect(d["snapshot"], None)
    assert any(o.tab_label == "Billing API" for o in sw.orchestrators)


def test_a_resumed_old_session_is_no_longer_followed(tmp_path, monkeypatch):
    """#38's Low: the old session was resumed later (conversation lines after its continued-in
    record): it is live again, the continuation no longer counts. Bookkeeping records after
    the continued-in (last-prompt, cost-state, as captured) do not count as a resume."""
    import os
    from herdr_panorama.scan import discovery
    from .scenario import S1
    d = build_demo(tmp_path)
    monkeypatch.setenv("PANORAMA_EVENTS_DIR", str(d["events"]))
    root = discovery.find_root(S1, None, str(d["projects"]))
    other = "66666666-6666-4666-8666-666666666666"
    with open(os.path.join(os.path.dirname(root), f"{other}.jsonl"), "w") as fh:
        fh.write(f'{{"type":"ai-title","aiTitle":"elsewhere","sessionId":"{other}"}}\n')
    with open(root, "a") as fh:
        fh.write(f'{{"type":"continued-in","timestamp":"2026-10-16T18:41:47.545Z","sessionId":"{S1}",'
                 f'"continuedInSessionId":"{other}"}}\n'
                 f'{{"type":"last-prompt","lastPrompt":"x","sessionId":"{S1}"}}\n'
                 f'{{"type":"cost-state","sessionId":"{S1}","totalCostUSD":1.0}}\n')
    c = Collector(root=str(d["projects"]), now=lambda: NOW, done_window=6 * 3600)
    billing = lambda sw: next(o for o in sw.orchestrators if o.pane_id == "w1:p1")  # noqa: E731
    assert billing(c.collect(d["snapshot"], None)).session_id == other
    with open(root, "a") as fh:  # `claude --resume <old>`: the conversation goes on in the old file
        fh.write(f'{{"parentUuid":null,"isSidechain":false,"type":"user","message":{{"role":"user","content":[{{"type":"text","text":"go on"}}]}},'
                 f'"uuid":"u1","timestamp":"2026-10-16T19:00:00.000Z","sessionId":"{S1}"}}\n')
    assert billing(c.collect(d["snapshot"], None)).session_id == S1


@pytest.mark.parametrize("bad", ["../../outside", "../x", "*", "/etc/passwd", "/tmp/elsewhere/x"])
def test_a_continuation_that_is_not_a_session_id_is_ignored(tmp_path, monkeypatch, bad):
    """#40 review: continuedInSessionId comes from a transcript (a remote machine's, too): only
    a UUID is ever followed, so no record can make the collector read a file outside the
    transcripts folder; find_root refuses anything else as well."""
    import os
    from herdr_panorama.scan import discovery
    from .scenario import S1
    d = build_demo(tmp_path / "t")
    monkeypatch.setenv("PANORAMA_EVENTS_DIR", str(d["events"]))
    outside = tmp_path / "outside.jsonl"  # what "../../outside" from a project dir would name
    outside.write_text('{"type":"ai-title","aiTitle":"not a session","sessionId":"x"}\n')
    root = discovery.find_root(S1, None, str(d["projects"]))
    with open(root, "a") as fh:
        fh.write('{"type":"continued-in","sessionId":"%s","continuedInSessionId":%s}\n' % (S1, json.dumps(bad)))
    c = Collector(root=str(d["projects"]), now=lambda: NOW, done_window=6 * 3600)
    sw = c.collect(d["snapshot"], None)
    billing = next(o for o in sw.orchestrators if o.pane_id == "w1:p1")
    assert billing.session_id == S1 and c.sessions[S1].continued_in is None
    assert discovery.find_root(bad, None, str(d["projects"])) is None
    assert discovery.find_root(str(outside)[:-len(".jsonl")], None, str(d["projects"])) is None
    assert os.path.isfile(outside)
