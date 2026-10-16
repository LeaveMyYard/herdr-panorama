"""End to end over the demo scenario: snapshot + transcripts + workflow + explicit channel + inbox."""

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
