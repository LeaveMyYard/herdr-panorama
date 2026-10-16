"""Unseen (design turn 6): a new failure, question or finished agent pulses until the operator
looks at it. Golden tests against docs/design/swarm-unseen.js rendered by render-6.mjs
(tests/fixtures/design_6.json), for the 2a graph (the docked panel's pulse is 9e's now:
tests/test_tiles.py); the tracker's rules and persistence; the seen triggers."""

import copy
import json

import pytest

from herdr_panorama import render, unseen
from herdr_panorama.model import DONE, FAIL, RUN, WAIT, Agent, Orchestrator, Swarm, Thread

from .design_dock import FIX, design_swarm
from .test_tui import disarm, loaded, make_app

FRAMES = json.loads((FIX / "design_6.json").read_text())["frames"]
MARKS = {"b3": "fail", "c1": "wait", "a5": "done", "i3": "wait", "k2": "done"}  # the design's sample


def norm(ch, fg, bg, bold):
    bg = (bg or render.BG).lower()
    return (ch, None, bg, None) if ch == " " else (ch, (fg or render.FG).lower(), bg, bool(bold))


def grid(runs):
    return [[norm(ch, r[1], r[2], r[3]) for r in row for ch in r[0]] for row in runs]


def cells(scr, w):
    return [[norm(c.ch, c.fg, c.bg, c.bold) for c in scr.c[y][:w]] for y in range(scr.h)]


def graph(frame, marks):
    v = render.View(orch="w1:p1", thread="t595", agent="a2", focus=2, frame=frame, unseen=dict(marks))
    return cells(render.render(design_swarm(), v, 160, 45), 160)


@pytest.mark.parametrize("frame", sorted(FRAMES, key=int))
def test_2a_pulses_the_same_cells_in_the_same_colours_as_the_design(frame):
    """Every cell the design's pulse changes (unseen vs nothing unseen) changes in the app, to
    the same character, colour and weight, and no other. Compared where the design and the app
    draw the same character without marks (their sample texts differ in a few tags)."""
    d_on, d_off = grid(FRAMES[frame]["G"]), grid(FRAMES[frame]["N"])
    a_on, a_off = graph(int(frame), MARKS), graph(int(frame), {})
    same = {(x, y) for y in range(45) for x in range(160) if d_off[y][x][0] == a_off[y][x][0]}
    assert len(same) > 6000
    want = {(x, y): d_on[y][x] for x, y in same if d_on[y][x] != d_off[y][x]}
    got = {(x, y): a_on[y][x] for x, y in same if a_on[y][x] != a_off[y][x]}
    assert len(want) > 400 and want == got


def test_the_pulse_is_slow_and_never_off():
    assert render.PULSE_PERIOD == 12  # 3 s at 250 ms frames
    full = [render.pulse_col(("fail", False), render.LINE, f) for f in range(12)]
    quiet = [render.pulse_col(("done", True), render.LINE, f) for f in range(12)]
    assert render.LINE not in full and render.LINE not in quiet
    assert full[6] == render.ST[FAIL] and full[0] == full[12 % 12]
    assert render.animated(Swarm(orchestrators=[orch([agent("x", DONE)])]), {"x": "done"})


# -- the tracker ----------------------------------------------------------------------------

def agent(aid, st, qids=(), ended=None):
    return Agent(id=aid, status=st, question_ids=list(qids), ended=ended)


def orch(agents, pane="w1:p1", focused=False):
    return Orchestrator(pane_id=pane, tab_id="t", tab_label="o", session_id="s", focused=focused,
                        threads=[Thread(id="t1", name="t", source="turn", agents=agents)])


def swarm(*agents, focused=False):
    return Swarm(orchestrators=[orch(list(agents), focused=focused)])


@pytest.fixture
def tracker(tmp_path):
    return unseen.Tracker(path=tmp_path / "unseen.json", enabled_fn=lambda: True)


def test_the_first_look_marks_nothing_then_changes_are_events(tracker):
    assert tracker.update(swarm(agent("a", RUN), agent("b", DONE), agent("c", FAIL)), 100.0) == {}
    marks = tracker.update(swarm(agent("a", FAIL), agent("b", DONE), agent("c", FAIL), agent("d", RUN)), 101.0)
    assert marks == {"a": "fail"}
    marks = tracker.update(swarm(agent("a", FAIL), agent("d", WAIT, [7])), 102.0)
    assert marks == {"a": "fail", "d": "wait"}
    marks = tracker.update(swarm(agent("a", FAIL), agent("d", WAIT, [7, 8])), 103.0)
    assert marks == {"a": "fail", "d": "wait"}
    marks = tracker.update(swarm(agent("a", FAIL), agent("d", DONE)), 103.5)
    assert marks == {"a": "fail", "d": "done"}  # answered (seen there), then finished
    tracker.seen(["d"])
    marks = tracker.update(swarm(agent("a", FAIL), agent("d", DONE)), 104.0)
    assert marks == {"a": "fail"}


def test_most_urgent_wins_and_an_answered_question_clears(tracker):
    tracker.update(swarm(agent("a", RUN)), 1.0)
    assert tracker.update(swarm(agent("a", WAIT, [3])), 2.0) == {"a": "wait"}
    assert tracker.update(swarm(agent("a", RUN)), 3.0) == {}  # answered where it was asked: seen
    assert tracker.update(swarm(agent("a", DONE)), 4.0) == {"a": "done"}
    assert tracker.update(swarm(agent("a", FAIL)), 5.0) == {"a": "fail"}
    assert tracker.update(swarm(agent("a", FAIL), agent("b", RUN)), 6.0) == {"a": "fail"}


def test_marks_survive_a_restart_and_are_shared_by_every_panorama(tmp_path):
    path = tmp_path / "unseen.json"
    one = unseen.Tracker(path=path, enabled_fn=lambda: True)
    one.update(swarm(agent("a", RUN), agent("b", RUN)), 1.0)
    one.update(swarm(agent("a", FAIL), agent("b", DONE)), 2.0)
    two = unseen.Tracker(path=path, enabled_fn=lambda: True)  # a restart, or the home tab next to a popup
    assert two.update(swarm(agent("a", FAIL), agent("b", DONE)), 3.0) == {"a": "fail", "b": "done"}  # once
    two.save_seen(two.seen(["a"]))
    assert one.update(swarm(agent("a", FAIL), agent("b", DONE)), 4.0) == {"b": "done"}


def test_what_happened_while_panorama_was_closed_counts(tmp_path):
    path = tmp_path / "unseen.json"
    unseen.Tracker(path=path, enabled_fn=lambda: True).update(swarm(agent("a", RUN)), 100.0)
    later = unseen.Tracker(path=path, enabled_fn=lambda: True)
    marks = later.update(swarm(agent("a", RUN), agent("old", DONE, ended=50.0), agent("new", DONE, ended=150.0)),
                         200.0)
    assert marks == {"new": "done"}


def test_focus_moving_onto_the_orchestrator_pane_clears_its_marks(tracker):
    tracker.update(swarm(agent("a", RUN)), 1.0)
    assert tracker.update(swarm(agent("a", FAIL)), 2.0) == {"a": "fail"}
    assert tracker.update(swarm(agent("a", FAIL), focused=True), 3.0) == {}


def test_review_probe_a_failure_under_resting_focus_is_kept(tracker):
    """Review M (#23): the agent fails while its orchestrator's pane holds herdr's focus (the
    operator stepped away): focus did not move, so the mark stays, also once focus leaves."""
    tracker.update(swarm(agent("a", RUN), focused=True), 1.0)
    assert tracker.update(swarm(agent("a", FAIL), focused=True), 2.0) == {"a": "fail"}
    assert tracker.update(swarm(agent("a", FAIL), focused=True), 2.5) == {"a": "fail"}  # still resting there
    assert tracker.update(swarm(agent("a", FAIL), focused=False), 3.0) == {"a": "fail"}
    assert tracker.update(swarm(agent("a", FAIL), focused=True), 4.0) == {}  # now focus arrives


def test_review_probe_the_first_refresh_after_start_never_clears_by_focus(tmp_path):
    path = tmp_path / "unseen.json"
    before = unseen.Tracker(path=path, enabled_fn=lambda: True)
    before.update(swarm(agent("a", RUN)), 1.0)
    before.update(swarm(agent("a", FAIL)), 2.0)
    restarted = unseen.Tracker(path=path, enabled_fn=lambda: True)  # panorama starts with that pane focused
    assert restarted.update(swarm(agent("a", FAIL), focused=True), 3.0) == {"a": "fail"}
    assert restarted.update(swarm(agent("a", FAIL), agent("b", RUN), focused=True), 4.0) == {"a": "fail"}


def test_review_probe_the_cockpit_focus_on_the_docked_agent_is_not_looking(tracker):
    """Docked next to an orchestrator, its pane keeps the focus: its subagents' events stay
    marked; the explicit dock (``g``/Enter/click in the view) is what clears."""
    tracker.update(swarm(agent("a", RUN), agent("b", RUN), focused=True), 1.0)
    assert tracker.update(swarm(agent("a", DONE), agent("b", RUN), focused=True), 2.0) == {"a": "done"}
    assert tracker.update(swarm(agent("a", DONE), agent("b", WAIT, [4]), focused=True), 3.0) == {"a": "done",
                                                                                                "b": "wait"}


def test_remote_agents_are_keyed_by_machine_in_the_state_file(tracker):
    o = orch([agent("a", RUN)], pane="m1:w1:p1")
    o.machine, o.machine_id = "box", "m1"
    tracker.update(Swarm(orchestrators=[o]), 1.0)
    o.threads[0].agents[0].status = FAIL
    assert tracker.update(Swarm(orchestrators=[o]), 2.0) == {"a": "fail"}
    assert set(json.loads(tracker._path().read_text())["agents"]) == {"m1:a"}
    tracker.save_seen(tracker.seen(["a"]))
    assert json.loads(tracker._path().read_text())["agents"]["m1:a"]["ev"] is None


def test_turned_off_marks_nothing(tmp_path, monkeypatch):
    cfg = tmp_path / "cfg"
    cfg.mkdir()
    (cfg / "config.toml").write_text("highlight_unseen = false\n")
    monkeypatch.setenv("HERDR_PLUGIN_CONFIG_DIR", str(cfg))
    assert unseen.enabled() is False
    t = unseen.Tracker(path=tmp_path / "u.json")
    t.update(swarm(agent("a", RUN)), 1.0)
    assert t.update(swarm(agent("a", FAIL)), 2.0) == {}
    (cfg / "config.toml").write_text("")
    assert unseen.enabled() is True  # the default


def test_a_busy_lock_keeps_the_seen_mark_until_it_is_written(tracker, monkeypatch):
    tracker.update(swarm(agent("a", RUN)), 1.0)
    tracker.update(swarm(agent("a", FAIL)), 2.0)
    monkeypatch.setattr(tracker, "_locked", lambda fn: None)  # another panorama holds the lock
    tracker.save_seen(tracker.seen(["a"]))
    assert tracker.marks == {}
    monkeypatch.undo()
    assert tracker.update(swarm(agent("a", FAIL)), 3.0) == {}


def test_a_broken_state_file_starts_over(tracker):
    tracker._path().write_text("{not json")
    assert tracker.update(swarm(agent("a", FAIL)), 1.0) == {}


def test_old_agents_are_forgotten(tracker):
    tracker.update(swarm(agent("a", RUN)), 0.0)
    tracker.update(swarm(agent("b", RUN)), unseen.KEEP_SECONDS + 1)
    assert set(json.loads(tracker._path().read_text())["agents"]) == {"b"}


# -- seen in the view -----------------------------------------------------------------------

def failed(sw, aid):
    sw = copy.deepcopy(sw)
    for o in sw.orchestrators:
        for a in o.agents():
            if a.id == aid:
                a.status = FAIL
    return sw


async def test_click_enter_and_go_to_pane_mark_seen(fake_herdr):
    from .demo import demo_swarm
    app, holder = make_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        base = demo_swarm()
        app.apply(failed(base, "a1c3d90"))  # author in PR #42 fails
        assert app.view.unseen == {"a1c3d90": "fail"}
        assert "a1c3d90" in app.tracker.marks
        app.arrived.clear()
        app.click_target("agent", ("w1:p1", "ref:#42", "a1c3d90"), cell=(0, 0))
        assert app.view.unseen == {}
        nxt = failed(failed(base, "a1c3d90"), "a2f81c0")
        app.apply(nxt)
        assert app.view.unseen == {"a2f81c0": "fail"}
        app._point("w1:p1", "ref:#42", "a2f81c0")
        disarm(app)
        await pilot.press("enter")
        assert app.view.unseen == {}
        nxt = failed(nxt, "c1d2e3f")
        app.apply(nxt)
        assert app.view.unseen == {"c1d2e3f": "fail"}
        disarm(app)
        await pilot.press("g")  # its orchestrator's pane: everything of it is in view
        assert app.view.unseen == {}


async def test_a_finished_card_clears_its_folded_agents(fake_herdr):
    from .demo import demo_swarm
    app, _ = make_app()
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        app.tracker.marks = {"a3e1b22": "done"}
        app.view.unseen = dict(app.tracker.marks)
        app._point("w1:p1", "ref:#42", render.FIN_CARD + "ref:#42")
        disarm(app)
        await pilot.press("enter")
        assert app.view.unseen == {} and "ref:#42" in app.view.fin_open


async def test_review_l2_the_state_file_is_read_and_written_off_the_ui_thread(fake_herdr):
    import threading
    app, _ = make_app()
    threads = []
    real = app.tracker.update

    def spy(sw, now):
        threads.append(threading.current_thread() is threading.main_thread())
        return real(sw, now)
    app.tracker.update = spy
    saves = []
    real_save = app.tracker.save_seen
    app.tracker.save_seen = lambda ids: (saves.append(threading.current_thread() is threading.main_thread()),
                                         real_save(ids))
    async with app.run_test(size=(160, 45)) as pilot:
        await loaded(app, pilot)
        assert threads and not any(threads)  # the collect worker, never the UI thread
        app.tracker.marks = {"a1c3d90": "fail"}
        app.view.unseen = dict(app.tracker.marks)
        app.arrived.clear()
        app.click_target("agent", ("w1:p1", "ref:#42", "a1c3d90"), cell=(0, 0))
        assert app.view.unseen == {}
        await pilot.pause(0.1)
        assert saves == [False]
